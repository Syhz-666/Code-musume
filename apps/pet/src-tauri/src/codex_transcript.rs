use code_musume_contracts::{Agent, BanterKind, VisibleProgress};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

const MAX_LINE: usize = 128 * 1024;
const BATCH: usize = 512 * 1024;
const MAX_BACKLOG: u64 = 8 * 1024 * 1024;
const ANCHOR: usize = 64;

#[derive(Deserialize)]
struct Row<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    #[serde(borrow)]
    payload: &'a RawValue,
}

#[derive(Deserialize)]
struct Boundary<'a> {
    #[serde(rename = "type")]
    kind: Option<&'a str>,
    turn_id: Option<&'a str>,
}

#[derive(Deserialize)]
struct Message<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    role: Option<&'a str>,
    phase: Option<&'a str>,
    #[serde(borrow)]
    content: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Block<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Session<'a> {
    id: &'a str,
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub opened: u64,
    pub progress_messages: u64,
    pub final_messages: u64,
    pub last_issue: Option<&'static str>,
}

pub struct Transcript {
    path: PathBuf,
    session: String,
    turn: String,
    turn_matched: bool,
    offset: u64,
    anchor: Vec<u8>,
    partial: Vec<u8>,
    skip_line: bool,
}

impl Transcript {
    pub fn open(path: &str, session: &str, turn: &str) -> Result<Self, &'static str> {
        let home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|p| p.join(".codex")))
            .ok_or("codex-home-unavailable")?;
        Self::open_in(&home.join("sessions"), Path::new(path), session, turn)
    }

    fn open_in(root: &Path, path: &Path, session: &str, turn: &str) -> Result<Self, &'static str> {
        if uuid::Uuid::parse_str(session).is_err()
            || uuid::Uuid::parse_str(turn).is_err()
            || !local_absolute(path)
            || !rollout_name(path, session)
        {
            return Err("invalid-transcript-path");
        }
        let root = root.canonicalize().map_err(|_| "sessions-unavailable")?;
        let path = path.canonicalize().map_err(|_| "transcript-unavailable")?;
        let relative = path.strip_prefix(root).map_err(|_| "outside-sessions")?;
        let parts: Vec<_> = relative.iter().collect();
        if !local_absolute(&path)
            || !rollout_name(&path, session)
            || parts.len() != 4
            || !parts[..3].iter().zip([4, 2, 2]).all(|(part, len)| {
                part.to_str()
                    .is_some_and(|s| s.len() == len && s.bytes().all(|b| b.is_ascii_digit()))
            })
            || !path.is_file()
        {
            return Err("invalid-transcript-path");
        }
        let mut file = File::open(&path).map_err(|_| "transcript-unavailable")?;
        let mut transcript = Self {
            path,
            session: session.into(),
            turn: turn.into(),
            turn_matched: false,
            offset: 0,
            anchor: Vec::new(),
            partial: Vec::new(),
            skip_line: false,
        };
        transcript.baseline(&mut file)?;
        Ok(transcript)
    }

    fn baseline(&mut self, file: &mut File) -> Result<(), &'static str> {
        let len = file.metadata().map_err(|_| "transcript-read")?.len();
        self.verify_header(file)?;
        // Inspect only framing and turn boundaries before EOF. Existing text is never replayed.
        let start = len.saturating_sub(MAX_LINE as u64);
        file.seek(SeekFrom::Start(start))
            .map_err(|_| "transcript-read")?;
        let mut bytes = Vec::new();
        file.take(len - start)
            .read_to_end(&mut bytes)
            .map_err(|_| "transcript-read")?;
        self.turn_matched = false;
        for line in bytes
            .split_inclusive(|b| *b == b'\n')
            .skip(usize::from(start > 0))
        {
            if line.ends_with(b"\n") {
                if let Ok(row) = serde_json::from_slice(&line[..line.len() - 1]) {
                    if let Some(matches) = turn_boundary(&row, &self.turn) {
                        self.turn_matched = matches;
                    }
                }
            }
        }
        self.offset = start + bytes.len() as u64;
        self.anchor = bytes[bytes.len().saturating_sub(ANCHOR)..].to_vec();
        self.partial.clear();
        self.skip_line = bytes.last().is_some_and(|b| *b != b'\n');
        Ok(())
    }

    fn verify_header(&self, file: &mut File) -> Result<(), &'static str> {
        file.seek(SeekFrom::Start(0))
            .map_err(|_| "transcript-read")?;
        let mut header = Vec::new();
        BufReader::new((&mut *file).take(MAX_LINE as u64))
            .read_until(b'\n', &mut header)
            .map_err(|_| "transcript-read")?;
        let meta = || {
            if !header.ends_with(b"\n") {
                return None;
            }
            let row: Row<'_> = serde_json::from_slice(&header).ok()?;
            (row.kind == "session_meta")
                .then(|| serde_json::from_str::<Session<'_>>(row.payload.get()).ok())
                .flatten()
        };
        if !meta().is_some_and(|m| m.id == self.session) {
            return Err("transcript-session-unverified");
        }

        Ok(())
    }

    pub fn read(&mut self) -> Result<(Vec<VisibleProgress>, Option<&'static str>), &'static str> {
        if self.path.canonicalize().as_ref().ok() != Some(&self.path) {
            return Err("transcript-path-changed");
        }
        let mut file = File::open(&self.path).map_err(|_| "transcript-unavailable")?;
        let metadata = file.metadata().map_err(|_| "transcript-read")?;
        if !metadata.is_file() {
            return Err("invalid-transcript-path");
        }
        self.verify_header(&mut file)?;
        let len = metadata.len();
        let issue = if len < self.offset || !self.anchor_matches(&mut file).unwrap_or(false) {
            Some("transcript-rewritten")
        } else if len - self.offset > MAX_BACKLOG {
            Some("transcript-backlog")
        } else {
            None
        };
        if issue.is_some() {
            self.baseline(&mut file)?;
            return Ok((Vec::new(), issue));
        }
        let mut bytes = Vec::new();
        file.take(BATCH as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "transcript-read")?;
        self.offset += bytes.len() as u64;
        self.anchor
            .extend_from_slice(&bytes[bytes.len().saturating_sub(ANCHOR)..]);
        if self.anchor.len() > ANCHOR {
            self.anchor.drain(..self.anchor.len() - ANCHOR);
        }

        let mut output = Vec::new();
        let mut issue = None;
        for fragment in bytes.split_inclusive(|b| *b == b'\n') {
            let complete = fragment.ends_with(b"\n");
            if !self.skip_line && self.partial.len() + fragment.len() <= MAX_LINE {
                self.partial.extend_from_slice(fragment);
                if complete {
                    if let Ok(row) = serde_json::from_slice(&self.partial) {
                        if let Some(matches) = turn_boundary(&row, &self.turn) {
                            self.turn_matched = matches;
                        } else if self.turn_matched && row.kind == "response_item" {
                            if let Some(message) = self.visible(row.payload) {
                                output.push(message);
                            }
                        }
                    }
                }
            } else if !self.skip_line {
                self.skip_line = true;
                issue = Some("transcript-line-too-large");
            }
            if complete {
                self.partial.clear();
                self.skip_line = false;
            }
        }
        Ok((output, issue))
    }

    fn anchor_matches(&self, file: &mut File) -> io::Result<bool> {
        file.seek(SeekFrom::Start(self.offset - self.anchor.len() as u64))?;
        let mut bytes = [0; ANCHOR];
        file.read_exact(&mut bytes[..self.anchor.len()])?;
        Ok(bytes[..self.anchor.len()] == self.anchor)
    }

    fn visible(&self, payload: &RawValue) -> Option<VisibleProgress> {
        let message: Message<'_> = serde_json::from_str(payload.get()).ok()?;
        if message.kind != "message" || message.role != Some("assistant") {
            return None;
        }
        let kind = match message.phase? {
            "commentary" => BanterKind::Progress,
            "final_answer" => BanterKind::Completion,
            _ => return None,
        };
        let blocks: Vec<Block<'_>> = serde_json::from_str(message.content?.get()).ok()?;
        let text = blocks
            .into_iter()
            .filter(|block| block.kind == "output_text")
            .filter_map(|block| block.text)
            .collect::<Vec<_>>()
            .join("\n");
        (!text.trim().is_empty()).then(|| VisibleProgress {
            agent: Agent::Codex,
            session: self.session.clone(),
            turn: self.turn.clone(),
            // The host derives a cross-source ID from the visible text and turn.
            id: String::new(),
            text,
            kind,
        })
    }
}

fn turn_boundary(row: &Row<'_>, turn: &str) -> Option<bool> {
    match row.kind {
        "turn_context" | "event_msg" => {
            let marker = serde_json::from_str::<Boundary<'_>>(row.payload.get()).ok()?;
            match (row.kind, marker.kind) {
                ("turn_context", _) | (_, Some("task_started")) => {
                    Some(marker.turn_id == Some(turn))
                }
                (_, Some("task_complete" | "turn_aborted")) => Some(false),
                _ => None,
            }
        }
        // Compaction can introduce carried history. Require a fresh turn marker after it.
        "session_meta" | "compacted" => Some(false),
        _ => None,
    }
}

fn rollout_name(path: &Path, session: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.starts_with("rollout-") && name.ends_with(&format!("-{session}.jsonl"))
        })
}

fn local_absolute(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    #[cfg(windows)]
    return matches!(
        path.components().next(),
        Some(std::path::Component::Prefix(prefix))
            if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_))
    );
    #[cfg(not(windows))]
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{fs, io::Write};

    const SESSION: &str = "6ef54896-baf1-4c6d-8acd-3eaacc70db3e";
    const TURN: &str = "916cea22-f89d-4a74-a47c-3cb3a3d85393";
    const ROWS: &str = include_str!("../../../../fixtures/codex-sync/rollout-0.171.jsonl");

    struct Fixture {
        home: PathBuf,
        path: PathBuf,
    }

    impl Fixture {
        fn new(initial: &str) -> Self {
            let home =
                std::env::temp_dir().join(format!("musume-rollout-{}", uuid::Uuid::new_v4()));
            let day = home.join("sessions/2026/11/25");
            fs::create_dir_all(&day).unwrap();
            let path = day.join(format!("rollout-2026-11-25T14-00-00-{SESSION}.jsonl"));
            fs::write(&path, initial).unwrap();
            Self { home, path }
        }

        fn open(&self, turn: &str) -> Result<Transcript, &'static str> {
            Transcript::open_in(&self.home.join("sessions"), &self.path, SESSION, turn)
        }

        fn append(&self, text: &str) {
            fs::OpenOptions::new()
                .append(true)
                .open(&self.path)
                .unwrap()
                .write_all(text.as_bytes())
                .unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.home);
        }
    }

    #[test]
    fn follows_verified_turn_and_filters_private_rows_and_oversized_tool_output() {
        let rows: Vec<_> = ROWS.lines().collect();
        let fixture = Fixture::new(&format!("{}\n", rows[..3].join("\n")));
        let mut tail = fixture.open(TURN).unwrap();
        let mid = rows[3].len() / 2;
        // Choose a valid UTF-8 split before the Chinese text.
        let mid = (0..=mid)
            .rev()
            .find(|i| rows[3].is_char_boundary(*i))
            .unwrap();
        fixture.append(&rows[3][..mid]);
        assert!(tail.read().unwrap().0.is_empty());
        fixture.append(&format!("{}\n{}\n", &rows[3][mid..], rows[4..6].join("\n")));
        let (messages, issue) = tail.read().unwrap();
        assert_eq!(issue, None);
        assert!(
            matches!(&messages[..], [p] if p.kind == BanterKind::Progress
            && p.text == "正在检查登录测试。" && p.turn == TURN)
        );
        fixture.append(&format!("{}\n", json!({
            "type":"response_item","payload":{"type":"function_call_output","output":"A".repeat(BATCH)}
        })));
        fixture.append(&format!("{}\n", rows[6..].join("\n")));
        let (messages, issue) = tail.read().unwrap();
        assert!(messages.is_empty());
        assert_eq!(issue, Some("transcript-line-too-large"));
        let (messages, _) = tail.read().unwrap();
        assert!(
            matches!(&messages[..], [p] if p.kind == BanterKind::Completion
            && p.text == "登录测试通过，保存设置还没验证。")
        );
        fixture.append(&format!("{}\n", rows[3]));
        assert!(tail.read().unwrap().0.is_empty());
    }

    #[test]
    fn never_replays_the_baseline_and_requires_identity_after_rewrite() {
        let rows: Vec<_> = ROWS.lines().collect();
        let prefix = format!("{}\n", rows[..4].join("\n"));
        let fixture = Fixture::new(&prefix);
        let mut tail = fixture.open(TURN).unwrap();
        assert!(tail.read().unwrap().0.is_empty());
        fixture.append(&format!("{}\n", rows[6]));
        assert_eq!(tail.read().unwrap().0.len(), 1);

        // Replacement is already longer than the old cursor when the next poll runs.
        let replacement = format!("{prefix}{}\n{}\n", rows[6].replace("保存", "导出"), rows[7]);
        fs::write(&fixture.path, replacement).unwrap();
        let (messages, issue) = tail.read().unwrap();
        assert!(messages.is_empty());
        assert_eq!(issue, Some("transcript-rewritten"));
        fixture.append(&format!("{}\n", rows[3]));
        assert!(tail.read().unwrap().0.is_empty());

        fs::write(&fixture.path, prefix.replace(SESSION, TURN)).unwrap();
        assert!(matches!(tail.read(), Err("transcript-session-unverified")));
        assert!(matches!(
            fixture.open(TURN),
            Err("transcript-session-unverified")
        ));
    }

    #[test]
    fn waits_for_matching_turn_and_drops_text_after_an_unrelated_boundary() {
        let rows: Vec<_> = ROWS.lines().collect();
        let fixture = Fixture::new(&format!("{}\n", rows[0]));
        let mut tail = fixture.open(TURN).unwrap();
        let wrong = uuid::Uuid::new_v4().to_string();
        fixture.append(&format!("{}\n{}\n", rows[1].replace(TURN, &wrong), rows[3]));
        assert!(tail.read().unwrap().0.is_empty());
        fixture.append(&format!("{}\n", rows[1..4].join("\n")));
        assert_eq!(tail.read().unwrap().0.len(), 1);
        fixture.append(&format!("{}\n{}\n", rows[2].replace(TURN, &wrong), rows[6]));
        assert!(tail.read().unwrap().0.is_empty());
    }
}
