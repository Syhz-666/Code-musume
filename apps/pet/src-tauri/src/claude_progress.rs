use code_musume_contracts::{Agent, VisibleProgress};
use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

const BATCH: usize = 128 * 1024;

pub struct Transcript {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
    skip_line: bool,
    turn: String,
    current_turn: bool,
}

impl Transcript {
    pub fn open(path: &str, session: &str, turn: &str) -> Option<Self> {
        let path = Path::new(path);
        if uuid::Uuid::parse_str(session).is_err()
            || !path.is_absolute()
            || path.file_name()?.to_str()? != format!("{session}.jsonl")
        {
            return None;
        }
        let root = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join(".claude"))
            })?;
        let root = root.join("projects").canonicalize().ok()?;
        let path = path.canonicalize().ok()?;
        #[cfg(windows)]
        if !matches!(
            path.components().next(),
            Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::VerbatimDisk(_) | std::path::Prefix::Disk(_))
        ) {
            return None;
        }
        // Resolve junctions before checking the expected main-transcript location.
        if path.strip_prefix(&root).ok()?.components().count() != 2 || !path.is_file() {
            return None;
        }
        let mut file = File::open(&path).ok()?;
        let offset = file.seek(SeekFrom::End(0)).ok()?;
        let skip_line = if offset > 0 {
            file.seek(SeekFrom::End(-1)).ok()?;
            let mut byte = [0];
            file.read_exact(&mut byte).ok()?;
            byte[0] != b'\n'
        } else {
            false
        };
        Some(Self {
            path,
            offset,
            partial: Vec::new(),
            skip_line,
            turn: turn.into(),
            current_turn: true,
        })
    }

    pub fn turn(&self) -> &str {
        &self.turn
    }

    pub fn read(&mut self, session: &str) -> std::io::Result<Vec<VisibleProgress>> {
        if self.path.canonicalize()? != self.path {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        let mut file = File::open(&self.path)?;
        let len = file.metadata()?.len();
        if len < self.offset || len - self.offset > BATCH as u64 * 8 {
            // Treat a rewrite or a long gap as a new baseline.
            self.offset = len;
            self.partial.clear();
            self.current_turn = false;
            self.skip_line = if len > 0 {
                file.seek(SeekFrom::End(-1))?;
                let mut byte = [0];
                file.read_exact(&mut byte)?;
                byte[0] != b'\n'
            } else {
                false
            };
            return Ok(Vec::new());
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.take(BATCH as u64).read_to_end(&mut bytes)?;
        self.offset += bytes.len() as u64;
        let mut output = Vec::new();
        for line in bytes.split_inclusive(|b| *b == b'\n') {
            let complete = line.ends_with(b"\n");
            if !self.skip_line && self.partial.len() + line.len() <= BATCH {
                self.partial.extend(line);
                if complete {
                    if let Ok(row) = serde_json::from_slice::<Value>(&self.partial) {
                        if row["type"] == "user"
                            && row["isSidechain"] == false
                            && row["sessionId"].as_str() == Some(session)
                        {
                            if let Some(turn) = row["promptId"].as_str() {
                                self.current_turn = turn == self.turn;
                            }
                        }
                        if self.current_turn {
                            if let Some(p) = extract(&row, session, &self.turn) {
                                output.push(p);
                            }
                        }
                    }
                }
            } else {
                self.skip_line = true;
            }
            if complete {
                self.partial.clear();
                self.skip_line = false;
            }
        }
        Ok(output)
    }
}

fn extract(row: &Value, session: &str, turn: &str) -> Option<VisibleProgress> {
    let message = &row["message"];
    if row["type"] != "assistant"
        || row["isSidechain"] != false
        || row["sessionId"].as_str()? != session
        || message["role"] != "assistant"
        || message["stop_reason"].as_str() != Some("tool_use")
    {
        return None;
    }
    let id = row["uuid"].as_str()?;
    uuid::Uuid::parse_str(id).ok()?;
    let text = message["content"]
        .as_array()?
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then(|| VisibleProgress {
        agent: Agent::ClaudeCode,
        session: session.into(),
        turn: turn.into(),
        id: id.into(),
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{fs, io::Write};

    #[test]
    fn waits_for_complete_lines_and_ignores_non_visible_blocks() {
        let session = uuid::Uuid::new_v4().to_string();
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("{session}.jsonl"));
        let mut file = File::create(&path).unwrap();
        let mut tail = Transcript {
            path: path.clone(),
            offset: 0,
            partial: Vec::new(),
            skip_line: false,
            turn: "turn".into(),
            current_turn: true,
        };
        let row = json!({
            "type":"assistant","sessionId":session,"uuid":uuid::Uuid::new_v4().to_string(),
            "isSidechain":false,"message":{"role":"assistant","stop_reason":"tool_use","content":[
                {"type":"thinking","thinking":"internal"},
                {"type":"tool_use","input":{"command":"raw"}},
                {"type":"text","text":"Checking the setting"}
            ]}
        });
        let bytes = serde_json::to_vec(&row).unwrap();
        file.write_all(&bytes[..bytes.len() / 2]).unwrap();
        assert!(tail.read(&session).unwrap().is_empty());
        file.write_all(&bytes[bytes.len() / 2..]).unwrap();
        writeln!(file).unwrap();
        assert_eq!(tail.read(&session).unwrap()[0].text, "Checking the setting");
        assert!(tail.read(&session).unwrap().is_empty());
        file.set_len(0).unwrap();
        assert!(tail.read(&session).unwrap().is_empty());
        let mut sidechain = row;
        sidechain["isSidechain"] = json!(true);
        assert!(extract(&sidechain, &session, "turn").is_none());
        drop(file);
        fs::remove_file(path).unwrap();
    }
}
