use crate::{adapter::short_hash, engine::MAX_SESSIONS};
use code_musume_contracts::{Agent, BanterProjection, VisibleProgress};
use std::collections::{BTreeMap, VecDeque};
use unicode_segmentation::UnicodeSegmentation;

const SEGMENT_CHARS: usize = 12_000;
const SEGMENT_MESSAGES: usize = 32;
const MAX_PROGRESS: usize = 4_000;
const INTERVAL: u64 = 30_000;
const QUIET: u64 = 120_000;
const RETENTION: u64 = 600_000;
const BUBBLE: u64 = 8_000;

#[derive(Clone, PartialEq, Eq)]
pub struct Target(pub Agent, pub String, pub String);

#[derive(Clone)]
pub enum Entry {
    Progress(String),
    Summary(String),
}
impl Entry {
    fn chars(&self) -> usize {
        match self {
            Self::Progress(s) | Self::Summary(s) => s.chars().count(),
        }
    }
}

#[derive(Default)]
struct Segment {
    entries: Vec<Entry>,
    seen: VecDeque<String>,
    version: u64,
    attempted: u64,
    updated: u64,
    last_summary: Option<String>,
    turn: String,
}

pub struct Request {
    pub id: u64,
    pub entries: Vec<Entry>,
}
struct InFlight {
    id: u64,
    target: Target,
    version: u64,
    eligible: bool,
}

pub struct Banter {
    enabled: bool,
    segments: BTreeMap<(Agent, String), Segment>,
    target: Option<Target>,
    quiet_since: u64,
    last_request: Option<u64>,
    request_id: u64,
    in_flight: Option<InFlight>,
    view: BanterProjection,
    pub skipped_long: u32,
}

impl Banter {
    pub fn new(instance_id: String, enabled: bool) -> Self {
        Self {
            enabled,
            segments: BTreeMap::new(),
            target: None,
            quiet_since: 0,
            last_request: None,
            request_id: 0,
            in_flight: None,
            view: BanterProjection {
                instance_id,
                revision: 0,
                agent: None,
                session_label: None,
                text: None,
                expires_at_ms: None,
            },
            skipped_long: 0,
        }
    }
    pub fn reset(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.segments.clear();
        self.in_flight = None;
        self.target = None;
        self.clear();
    }
    pub fn snapshot(&self) -> BanterProjection {
        self.view.clone()
    }
    pub fn retain(&mut self, keep: impl Fn(&(Agent, String)) -> bool) {
        self.segments.retain(|key, _| keep(key));
    }
    pub fn ingest(&mut self, p: VisibleProgress, now: u64) {
        if !self.enabled || p.text.trim().is_empty() {
            return;
        }
        if p.text.chars().count() > MAX_PROGRESS {
            self.skipped_long = self.skipped_long.saturating_add(1);
            return;
        }
        let key = (p.agent, p.session);
        if !self.segments.contains_key(&key) && self.segments.len() >= MAX_SESSIONS {
            return;
        }
        let segment = self.segments.entry(key.clone()).or_default();
        if segment.seen.contains(&p.id) {
            return;
        }
        if segment.seen.len() == 512 {
            segment.seen.pop_front();
        }
        segment.seen.push_back(p.id);
        let entry = Entry::Progress(p.text);
        // Reserve space for the answer before creating the request prefix.
        if segment.entries.len() + 2 > SEGMENT_MESSAGES
            || segment.entries.iter().map(Entry::chars).sum::<usize>() + entry.chars() + 256
                > SEGMENT_CHARS
        {
            let carry: Vec<_> = segment
                .entries
                .iter()
                .rev()
                .filter(|e| matches!(e, Entry::Progress(_)))
                .take(2)
                .cloned()
                .collect();
            segment.entries.clear();
            segment.entries.extend(carry.into_iter().rev());
            if segment.entries.iter().map(Entry::chars).sum::<usize>() + entry.chars() + 256
                > SEGMENT_CHARS
            {
                segment.entries.remove(0);
            }
        }
        segment.entries.push(entry);
        segment.turn = p.turn.clone();
        segment.version += 1;
        segment.updated = now;
        if self
            .target
            .as_ref()
            .is_some_and(|t| t.0 == key.0 && t.1 == key.1 && t.2 == p.turn)
        {
            self.quiet_since = now;
            self.clear();
        }
    }
    pub fn tick(&mut self, target: Option<Target>, now: u64, can_request: bool) -> Option<Request> {
        self.segments
            .retain(|_, s| now.saturating_sub(s.updated) < RETENTION);
        let target = target.filter(|_| self.enabled);
        if self.target != target {
            self.target = target;
            self.quiet_since = now;
            if let Some(flight) = &mut self.in_flight {
                flight.eligible = false;
            }
            self.clear();
        }
        if self.view.expires_at_ms.is_some_and(|end| now >= end) {
            self.clear();
        }
        let target = self.target.clone()?;
        let segment = self.segments.get_mut(&(target.0, target.1.clone()));
        if let Some(segment) = segment {
            if can_request
                && self.in_flight.is_none()
                && segment.attempted != segment.version
                && segment.turn == target.2
                && self
                    .last_request
                    .is_none_or(|last| now.saturating_sub(last) >= INTERVAL)
            {
                segment.attempted = segment.version;
                self.last_request = Some(now);
                self.request_id += 1;
                self.in_flight = Some(InFlight {
                    id: self.request_id,
                    target,
                    version: segment.version,
                    eligible: true,
                });
                return Some(Request {
                    id: self.request_id,
                    entries: segment.entries.clone(),
                });
            }
        }
        if now.saturating_sub(self.quiet_since) >= QUIET {
            self.quiet_since = now;
            self.show(&target, "还没新进展，我陪你等会儿。".into(), now);
        }
        None
    }
    pub fn finish(&mut self, id: u64, text: Option<String>, now: u64) -> bool {
        if self.in_flight.as_ref().is_none_or(|f| f.id != id) {
            return false;
        }
        let flight = self.in_flight.take().unwrap();
        let Some(segment) = self
            .segments
            .get_mut(&(flight.target.0, flight.target.1.clone()))
        else {
            return true;
        };
        if flight.eligible && flight.version == segment.version {
            if let Some(text) = text.and_then(clean_summary) {
                if segment.last_summary.as_ref() != Some(&text) {
                    segment.last_summary = Some(text.clone());
                    segment.entries.push(Entry::Summary(text.clone()));
                    if self.target.as_ref() == Some(&flight.target) {
                        self.show(&flight.target, text, now);
                    }
                }
            }
        }
        true
    }
    fn show(&mut self, target: &Target, text: String, now: u64) {
        self.view.revision += 1;
        self.view.agent = Some(target.0);
        self.view.session_label = Some(short_hash(&target.1));
        self.view.text = Some(text);
        self.view.expires_at_ms = Some(now + BUBBLE);
    }
    fn clear(&mut self) {
        if self.view.text.take().is_some() {
            self.view.revision += 1;
            self.view.agent = None;
            self.view.session_label = None;
            self.view.expires_at_ms = None;
        }
    }
}

pub fn clean_summary(text: String) -> Option<String> {
    let trimmed = text.trim_matches(' ');
    if trimmed.is_empty()
        || trimmed.graphemes(true).count() > 20
        || trimmed.chars().count() > 256
        || trimmed.starts_with(['"', '\'', '“', '「', '#', '-', '*', '>'])
        || trimmed.contains(['`', '<', '>'])
        || trimmed.chars().any(char::is_control)
    {
        return None;
    }
    Some(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(b: &mut Banter, agent: Agent, turn: &str, id: &str, text: &str, now: u64) {
        b.ingest(
            VisibleProgress {
                agent,
                session: "same-session".into(),
                turn: turn.into(),
                id: id.into(),
                text: text.into(),
            },
            now,
        );
    }
    fn target(agent: Agent, turn: &str) -> Option<Target> {
        Some(Target(agent, "same-session".into(), turn.into()))
    }

    #[test]
    fn prefix_is_append_only_and_switching_sources_invalidates_inflight_display() {
        let mut b = Banter::new("test".into(), true);
        let codex = target(Agent::Codex, "one");
        let claude = target(Agent::ClaudeCode, "one");
        progress(&mut b, Agent::Codex, "one", "a", "checking settings", 0);
        let first = b.tick(codex.clone(), 0, true).unwrap();
        assert!(matches!(&first.entries[..], [Entry::Progress(t)] if t == "checking settings"));
        b.finish(first.id, Some("还在检查设置呢。".into()), 20);
        assert_eq!(b.snapshot().expires_at_ms, Some(8020));
        b.tick(codex.clone(), 8020, false);
        assert_eq!(b.snapshot().text, None);
        progress(&mut b, Agent::Codex, "one", "a", "duplicate", 8050);
        progress(&mut b, Agent::Codex, "one", "b", "settings fixed", 8100);
        assert!(b.tick(codex.clone(), 29_999, true).is_none());
        let second = b.tick(codex, 30_000, true).unwrap();
        assert!(
            matches!(&second.entries[..], [Entry::Progress(a), Entry::Summary(s), Entry::Progress(b)]
            if a == "checking settings" && s == "还在检查设置呢。" && b == "settings fixed")
        );
        progress(
            &mut b,
            Agent::ClaudeCode,
            "one",
            "a",
            "running tests",
            30_050,
        );
        b.tick(claude.clone(), 30_050, true);
        b.finish(second.id, Some("设置修好啦。".into()), 30_100);
        assert_eq!(b.snapshot().text, None);
        let third = b.tick(claude, 60_000, true).unwrap();
        assert!(matches!(&third.entries[..], [Entry::Progress(t)] if t == "running tests"));
        b.reset(false);
        assert!(!b.finish(third.id, Some("这条不应出现。".into()), 60_100));
    }

    #[test]
    fn quiet_bubble_and_session_retention_follow_virtual_time() {
        let mut b = Banter::new("test".into(), true);
        let first = target(Agent::Codex, "one");
        b.tick(first.clone(), 0, true);
        b.tick(first.clone(), 119_999, true);
        assert!(b.snapshot().text.is_none());
        assert!(b.tick(first.clone(), 120_000, true).is_none());
        assert!(b.snapshot().text.is_some());
        assert!(b.tick(None, 120_001, true).is_none());
        assert!(b.snapshot().text.is_none());
        progress(&mut b, Agent::Codex, "one", "a", "test failed", 120_002);
        assert!(b.tick(target(Agent::Codex, "two"), 120_003, true).is_none());
        b.tick(None, 720_002, true);
        assert!(b.segments.is_empty());
    }

    #[test]
    fn segments_roll_over_without_carrying_generated_claims() {
        let mut b = Banter::new("test".into(), true);
        for (index, ch) in ['甲', '乙', '丙', '丁'].into_iter().enumerate() {
            progress(
                &mut b,
                Agent::Codex,
                "one",
                &index.to_string(),
                &ch.to_string().repeat(3100),
                index as u64,
            );
        }
        let result = b.tick(target(Agent::Codex, "one"), 5, true).unwrap();
        assert_eq!(result.entries.len(), 3);
        assert!(matches!(&result.entries[0], Entry::Progress(s) if s.starts_with('乙')));
        assert!(matches!(&result.entries[2], Entry::Progress(s) if s.starts_with('丁')));
    }

    #[test]
    fn counts_graphemes_and_rejects_multiline_output() {
        assert!(clean_summary("👩‍💻".repeat(20)).is_some());
        assert!(clean_summary("👩‍💻".repeat(21)).is_none());
        assert!(clean_summary("第一行\n第二行".into()).is_none());
    }
}
