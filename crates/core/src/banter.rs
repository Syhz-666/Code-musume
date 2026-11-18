use crate::{adapter::short_hash, engine::MAX_SESSIONS};
use code_musume_contracts::{Agent, BanterKind, BanterProjection, VisibleProgress};
use std::collections::{BTreeMap, VecDeque};
use unicode_segmentation::UnicodeSegmentation;
mod idle;
use idle::IdleBanter;
pub use idle::{IdleDiagnostics, IdleGate};

const SEGMENT_CHARS: usize = 12_000;
const SEGMENT_MESSAGES: usize = 32;
const MAX_PROGRESS: usize = 4_000;
const INTERVAL: u64 = 30_000;
const QUIET: u64 = 120_000;
const QUIET_FALLBACK: &str = "还没新进展，我陪你等会儿。";
const RETENTION: u64 = 600_000;
const BUBBLE: u64 = 8_000;
const FINAL_WINDOW: u64 = 2_000;
const COMPLETION_DEADLINE: u64 = 20_000;

#[derive(Clone, PartialEq, Eq)]
pub struct Target(pub Agent, pub String, pub String);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CompletionGate {
    Waiting,
    Idle,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuietActivity {
    Working,
    Thinking,
    AwaitingUpdate,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    Observe,
    Unavailable,
    Ready,
}

#[derive(Clone)]
pub enum Entry {
    Progress(String),
    Final(String),
    Summary(String),
    Idle {
        recent: Vec<String>,
    },
    Quiet {
        activity: QuietActivity,
        recent_progress: Option<String>,
        previous_line: Option<String>,
    },
}
impl Entry {
    fn chars(&self) -> usize {
        match self {
            Self::Progress(s) | Self::Final(s) | Self::Summary(s) => s.chars().count(),
            Self::Idle { recent } => recent.iter().map(|s| s.chars().count()).sum(),
            Self::Quiet {
                recent_progress,
                previous_line,
                ..
            } => recent_progress
                .iter()
                .chain(previous_line)
                .map(|text| text.chars().count())
                .sum(),
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Progress,
    Completion,
    Quiet(QuietActivity),
    Idle,
}
struct InFlight {
    id: u64,
    target: Option<Target>,
    version: u64,
    eligible: bool,
    kind: RequestKind,
}

struct Completion {
    target: Target,
    ended_at: u64,
    requested: bool,
    result: Option<String>,
    shown: bool,
}

pub struct Banter {
    enabled: bool,
    idle: IdleBanter,
    segments: BTreeMap<(Agent, String), Segment>,
    target: Option<Target>,
    quiet_since: u64,
    last_quiet_line: Option<String>,
    last_request: Option<u64>,
    request_id: u64,
    in_flight: Option<InFlight>,
    finals: BTreeMap<(Agent, String), (String, String)>,
    completion: Option<Completion>,
    pub completion_phase: &'static str,
    pub completion_requests: u64,
    pub completion_shown: u64,
    view: BanterProjection,
    pub skipped_long: u32,
}

impl Banter {
    pub fn new(instance_id: String, enabled: bool) -> Self {
        Self {
            enabled,
            idle: IdleBanter::default(),
            segments: BTreeMap::new(),
            target: None,
            quiet_since: 0,
            last_quiet_line: None,
            last_request: None,
            request_id: 0,
            in_flight: None,
            finals: BTreeMap::new(),
            completion: None,
            completion_phase: "idle",
            completion_requests: 0,
            completion_shown: 0,
            view: BanterProjection {
                instance_id,
                revision: 0,
                agent: None,
                session_label: None,
                text: None,
                kind: None,
                expires_at_ms: None,
            },
            skipped_long: 0,
        }
    }
    pub fn reset(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.segments.clear();
        if !self.has_idle_request() {
            self.in_flight = None;
        }
        self.finals.clear();
        self.completion = None;
        self.completion_phase = "idle";
        self.target = None;
        self.last_quiet_line = None;
        if self.view.kind != Some(BanterKind::Idle) {
            self.clear();
        }
    }
    pub fn reset_idle(&mut self, enabled: bool) -> bool {
        self.idle = IdleBanter::default();
        self.idle.enabled = enabled;
        self.cancel_idle()
    }
    pub fn idle_resting(&self) -> Option<bool> {
        self.idle.enabled.then_some(self.idle.resting)
    }
    pub fn idle_diagnostics(&self) -> &IdleDiagnostics {
        &self.idle.diagnostics
    }
    pub fn has_idle_request(&self) -> bool {
        self.in_flight
            .as_ref()
            .is_some_and(|f| f.kind == RequestKind::Idle)
    }
    fn cancel_idle(&mut self) -> bool {
        if self.view.kind == Some(BanterKind::Idle) {
            self.clear();
        }
        let abort = self.has_idle_request();
        if abort {
            self.in_flight = None;
            self.idle.diagnostics.last_skip = Some("interrupted");
        }
        abort
    }
    pub fn update_idle(&mut self, mut gate: IdleGate, now: u64) -> bool {
        if !self.idle.enabled || self.completion.is_some() {
            gate = IdleGate::Blocked;
        }
        self.idle.update(gate, now);
        if gate != IdleGate::Ready {
            return self.cancel_idle();
        }
        if self.view.kind == Some(BanterKind::Idle)
            && self.view.expires_at_ms.is_some_and(|end| now >= end)
        {
            self.clear();
        }
        false
    }
    pub fn tick_idle(
        &mut self,
        now: u64,
        dispatch: Dispatch,
        dice: impl FnOnce() -> u8,
    ) -> Option<Request> {
        if dispatch == Dispatch::Observe || !self.idle.roll(now, dice) {
            return None;
        }
        if self.completion.is_some()
            || self.in_flight.is_some()
            || dispatch == Dispatch::Unavailable
        {
            self.idle.diagnostics.last_skip = Some(if dispatch == Dispatch::Unavailable {
                "service-unavailable"
            } else {
                "request-busy"
            });
            return None;
        }
        self.idle.diagnostics.requests += 1;
        self.request_id += 1;
        self.in_flight = Some(InFlight {
            id: self.request_id,
            target: None,
            version: 0,
            eligible: true,
            kind: RequestKind::Idle,
        });
        Some(Request {
            id: self.request_id,
            entries: vec![Entry::Idle {
                recent: self.idle.recent.iter().cloned().collect(),
            }],
        })
    }
    pub fn snapshot(&self) -> BanterProjection {
        self.view.clone()
    }
    pub fn retain(&mut self, keep: impl Fn(&(Agent, String)) -> bool) {
        self.segments.retain(|key, _| keep(key));
        self.finals.retain(|key, _| keep(key));
    }
    pub fn ingest(&mut self, p: VisibleProgress, now: u64) {
        if !self.enabled || p.kind == BanterKind::Idle || p.text.trim().is_empty() {
            return;
        }
        if p.text.chars().count() > MAX_PROGRESS {
            self.skipped_long = self.skipped_long.saturating_add(1);
            return;
        }
        let key = (p.agent, p.session);
        if p.kind == BanterKind::Completion {
            self.finals.insert(key, (p.turn, p.text));
            return;
        }
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
            if let Some(flight) = self.in_flight.as_mut().filter(|flight| {
                matches!(flight.kind, RequestKind::Quiet(_)) && flight.target == self.target
            }) {
                flight.eligible = false;
            }
            self.clear_progress();
        }
    }
    pub fn discard_final(&mut self, agent: Agent, session: &str) {
        self.finals.remove(&(agent, session.into()));
    }

    pub fn retain_finals(&mut self, mut keep: impl FnMut(Agent, &str, &str) -> bool) {
        self.finals
            .retain(|(agent, session), (turn, _)| keep(*agent, session, turn));
    }

    pub fn completed(&mut self, target: Target, now: u64) -> bool {
        if !self.enabled {
            return false;
        }
        let mut abort_task = self.cancel_completion("superseded");
        if self
            .in_flight
            .as_ref()
            .is_some_and(|f| f.target.as_ref() == Some(&target))
        {
            self.in_flight = None;
            abort_task = true;
        }
        self.completion = Some(Completion {
            target,
            ended_at: now,
            requested: false,
            result: None,
            shown: false,
        });
        self.completion_phase = "awaiting-final";
        abort_task
    }

    pub fn completion_target(&self) -> Option<&Target> {
        self.completion.as_ref().map(|c| &c.target)
    }

    pub fn accepts_final(&self, agent: Agent, session: &str, turn: &str, now: u64) -> bool {
        self.completion.as_ref().is_some_and(|c| {
            c.target.0 == agent
                && c.target.1 == session
                && c.target.2 == turn
                && !c.requested
                && now <= c.ended_at + FINAL_WINDOW
        })
    }

    pub fn cancel_completion(&mut self, reason: &'static str) -> bool {
        let Some(c) = self.completion.take() else {
            return false;
        };
        self.discard_final(c.target.0, &c.target.1);
        self.completion_phase = reason;
        if self.view.kind == Some(BanterKind::Completion) {
            self.clear();
        }
        let abort = self
            .in_flight
            .as_ref()
            .is_some_and(|f| f.kind == RequestKind::Completion);
        if abort {
            self.in_flight = None;
        }
        abort
    }

    pub fn tick_completion(
        &mut self,
        gate: CompletionGate,
        now: u64,
        can_request: bool,
    ) -> (Option<Request>, bool) {
        let Some(c) = self.completion.as_mut() else {
            return (None, false);
        };
        if gate == CompletionGate::Blocked {
            return (None, self.cancel_completion("preempted"));
        }
        if c.shown {
            if self.view.expires_at_ms.is_some_and(|end| now >= end) {
                self.cancel_completion("expired");
            }
            return (None, false);
        }
        if now >= c.ended_at + COMPLETION_DEADLINE {
            return (None, self.cancel_completion("deadline"));
        }
        if gate == CompletionGate::Idle && now >= c.ended_at + crate::engine::DONE_MS {
            if let Some(text) = c.result.take() {
                let target = c.target.clone();
                c.shown = true;
                self.completion_phase = "shown";
                self.completion_shown += 1;
                self.show(Some(&target), text, now, BanterKind::Completion);
                return (None, false);
            }
        }
        let key = (c.target.0, c.target.1.clone());
        let has_final = self
            .finals
            .get(&key)
            .is_some_and(|(turn, _)| *turn == c.target.2);
        if !c.requested && !has_final && now > c.ended_at + FINAL_WINDOW {
            return (None, self.cancel_completion("no-final"));
        }
        if !c.requested && has_final && can_request && self.in_flight.is_none() {
            let (_, text) = self.finals.remove(&key).unwrap();
            c.requested = true;
            self.request_id += 1;
            self.completion_phase = "requested";
            self.completion_requests += 1;
            self.in_flight = Some(InFlight {
                id: self.request_id,
                target: Some(c.target.clone()),
                version: 0,
                eligible: true,
                kind: RequestKind::Completion,
            });
            return (
                Some(Request {
                    id: self.request_id,
                    entries: vec![Entry::Final(text)],
                }),
                false,
            );
        }
        (None, false)
    }

    pub fn tick(
        &mut self,
        work: Option<(Target, QuietActivity)>,
        now: u64,
        dispatch: Dispatch,
    ) -> Option<Request> {
        self.segments
            .retain(|_, s| now.saturating_sub(s.updated) < RETENTION);
        let (target, activity) = work.filter(|_| self.enabled).unzip();
        if self.target != target {
            self.target = target;
            self.quiet_since = now;
            self.last_quiet_line = None;
            if let Some(flight) = self
                .in_flight
                .as_mut()
                .filter(|f| matches!(f.kind, RequestKind::Progress | RequestKind::Quiet(_)))
            {
                flight.eligible = false;
            }
            self.clear_progress();
        }
        if self.view.expires_at_ms.is_some_and(|end| now >= end) {
            self.clear_progress();
        }
        if let Some(flight) = self.in_flight.as_mut() {
            if let RequestKind::Quiet(start_activity) = flight.kind {
                flight.eligible &= activity == Some(start_activity);
            }
            return None;
        }
        let target = self.target.clone()?;
        let activity = activity?;
        if dispatch == Dispatch::Observe {
            return None;
        }
        let quiet_due = now.saturating_sub(self.quiet_since) >= QUIET;
        if dispatch == Dispatch::Unavailable {
            if quiet_due {
                self.quiet_since = now;
                self.show_quiet_fallback(&target, now);
            }
            return None;
        }
        if self
            .last_request
            .is_some_and(|last| now.saturating_sub(last) < INTERVAL)
        {
            return None;
        }
        let mut segment = self
            .segments
            .get_mut(&(target.0, target.1.clone()))
            .filter(|segment| segment.turn == target.2);
        if let Some(segment) = segment.as_deref_mut().filter(|segment| {
            activity != QuietActivity::AwaitingUpdate && segment.attempted != segment.version
        }) {
            segment.attempted = segment.version;
            let version = segment.version;
            let entries = segment.entries.clone();
            return Some(self.start_work_request(
                target,
                version,
                RequestKind::Progress,
                entries,
                now,
            ));
        }
        if !quiet_due {
            return None;
        }
        let recent_progress = segment.and_then(|s| {
            s.entries.iter().rev().find_map(|entry| match entry {
                Entry::Progress(text) => Some(text.clone()),
                _ => None,
            })
        });
        self.quiet_since = now;
        let entries = vec![Entry::Quiet {
            activity,
            recent_progress,
            previous_line: self.last_quiet_line.clone(),
        }];
        Some(self.start_work_request(target, 0, RequestKind::Quiet(activity), entries, now))
    }
    fn start_work_request(
        &mut self,
        target: Target,
        version: u64,
        kind: RequestKind,
        entries: Vec<Entry>,
        now: u64,
    ) -> Request {
        self.last_request = Some(now);
        self.request_id += 1;
        self.in_flight = Some(InFlight {
            id: self.request_id,
            target: Some(target),
            version,
            eligible: true,
            kind,
        });
        Request {
            id: self.request_id,
            entries,
        }
    }
    pub fn quiet_activity(&self, id: u64) -> Option<QuietActivity> {
        match self.in_flight.as_ref().filter(|f| f.id == id)?.kind {
            RequestKind::Quiet(activity) => Some(activity),
            _ => None,
        }
    }
    pub fn finish(&mut self, id: u64, text: Option<String>, now: u64) -> bool {
        if self.in_flight.as_ref().is_none_or(|f| f.id != id) {
            return false;
        }
        let flight = self.in_flight.take().unwrap();
        if flight.kind == RequestKind::Idle {
            match text.and_then(clean_summary) {
                Some(text) if !self.idle.recent.contains(&text) => {
                    if self.idle.recent.len() == 3 {
                        self.idle.recent.pop_front();
                    }
                    self.idle.recent.push_back(text.clone());
                    self.idle.diagnostics.shown += 1;
                    self.show(None, text, now, BanterKind::Idle);
                }
                Some(_) => self.idle.diagnostics.last_skip = Some("duplicate"),
                None => self.idle.diagnostics.last_skip = Some("no-line"),
            }
            return true;
        }
        if flight.kind == RequestKind::Completion {
            let summary = text.and_then(clean_summary);
            if summary.is_none() {
                self.cancel_completion("no-summary");
                return true;
            }
            if let Some(c) = self
                .completion
                .as_mut()
                .filter(|c| now < c.ended_at + COMPLETION_DEADLINE)
            {
                c.result = summary;
                self.completion_phase = "ready";
            }
            return true;
        }
        let target = flight.target.unwrap();
        if matches!(flight.kind, RequestKind::Quiet(_)) {
            if flight.eligible {
                if let Some(text) = text.and_then(clean_summary) {
                    if self.last_quiet_line.as_ref() != Some(&text) {
                        self.last_quiet_line = Some(text.clone());
                        self.show(Some(&target), text, now, BanterKind::Progress);
                    }
                } else {
                    self.show_quiet_fallback(&target, now);
                }
            }
            return true;
        }
        let Some(segment) = self.segments.get_mut(&(target.0, target.1.clone())) else {
            return true;
        };
        if flight.eligible && flight.version == segment.version {
            if let Some(text) = text.and_then(clean_summary) {
                if segment.last_summary.as_ref() != Some(&text) {
                    segment.last_summary = Some(text.clone());
                    segment.entries.push(Entry::Summary(text.clone()));
                    if self.target.as_ref() == Some(&target) {
                        self.show(Some(&target), text, now, BanterKind::Progress);
                    }
                }
            }
        }
        true
    }
    fn show_quiet_fallback(&mut self, target: &Target, now: u64) {
        self.show(
            Some(target),
            QUIET_FALLBACK.into(),
            now,
            BanterKind::Progress,
        );
    }
    fn show(&mut self, target: Option<&Target>, text: String, now: u64, kind: BanterKind) {
        self.view.revision += 1;
        self.view.agent = target.map(|target| target.0);
        self.view.session_label = target.map(|target| short_hash(&target.1));
        self.view.text = Some(text);
        self.view.kind = Some(kind);
        self.view.expires_at_ms = Some(now + BUBBLE);
    }
    fn clear_progress(&mut self) {
        if self.view.kind == Some(BanterKind::Progress) {
            self.clear();
        }
    }
    fn clear(&mut self) {
        if self.view.text.take().is_some() {
            self.view.revision += 1;
            self.view.agent = None;
            self.view.kind = None;
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

    #[test]
    fn idle_dice_use_awake_and_rest_intervals_and_skip_missed_deadlines() {
        let mut b = Banter::new("test".into(), false);
        b.reset_idle(true);
        b.update_idle(IdleGate::Ready, 0);
        assert!(b
            .tick_idle(29_999, Dispatch::Ready, || panic!("too early"))
            .is_none());
        for (i, face) in [1, 1, 2, 1, 1, 1].into_iter().enumerate() {
            assert!(b
                .tick_idle(30_000 * (i as u64 + 1), Dispatch::Ready, || face)
                .is_none());
            assert_eq!(b.idle_resting(), Some(i == 5));
        }
        // Hidden while resting. Resume with a full interval and a new streak.
        assert!(b.tick_idle(195_000, Dispatch::Ready, || 6).is_none());
        b.update_idle(IdleGate::Paused, 196_000);
        b.update_idle(IdleGate::Ready, 220_000);
        assert_eq!(b.idle_resting(), Some(true));
        for (at, face) in [(235_000, 6), (250_000, 2), (265_000, 6), (280_000, 6)] {
            assert!(b.tick_idle(at, Dispatch::Ready, || face).is_none());
            assert_eq!(b.idle_resting(), Some(at < 280_000));
        }
        assert!(b
            .tick_idle(309_999, Dispatch::Ready, || panic!("too early"))
            .is_none());
        assert!(b
            .tick_idle(900_000, Dispatch::Ready, || panic!("missed deadline"))
            .is_none());
        assert!(b
            .tick_idle(900_000, Dispatch::Ready, || panic!("catch-up"))
            .is_none());
        assert!(b.tick_idle(930_000, Dispatch::Ready, || 6).is_some());
    }

    #[test]
    fn idle_lines_only_remember_displayed_text_and_cancel_on_work_or_completion() {
        let mut b = Banter::new("test".into(), true);
        b.reset_idle(true);
        b.update_idle(IdleGate::Ready, 0);
        progress(
            &mut b,
            Agent::Codex,
            "earlier",
            "p",
            "private work context",
            1,
        );
        assert!(b.tick_idle(30_000, Dispatch::Unavailable, || 6).is_none());
        let first = b.tick_idle(60_000, Dispatch::Ready, || 6).unwrap();
        assert!(matches!(&first.entries[..], [Entry::Idle { recent }] if recent.is_empty()));
        assert!(b.finish(first.id, Some("发梢又翘啦。".into()), 60_001));
        let shown = b.snapshot();
        assert_eq!(shown.kind, Some(BanterKind::Idle));
        assert_eq!(shown.agent, None);
        assert_eq!(shown.session_label, None);
        assert_eq!(shown.expires_at_ms, Some(68_001));
        b.update_idle(IdleGate::Ready, 68_001);
        assert!(b.snapshot().text.is_none());
        let duplicate = b.tick_idle(90_000, Dispatch::Ready, || 6).unwrap();
        assert!(
            matches!(&duplicate.entries[..], [Entry::Idle { recent }] if recent == &["发梢又翘啦。"])
        );
        assert!(b.finish(duplicate.id, Some("发梢又翘啦。".into()), 90_001));
        assert!(b.snapshot().text.is_none());
        let late = b.tick_idle(120_000, Dispatch::Ready, || 6).unwrap();
        assert!(b.update_idle(IdleGate::Blocked, 120_001));
        assert!(!b.finish(late.id, Some("这句已经过时。".into()), 120_002));
        b.update_idle(IdleGate::Ready, 121_000);
        let finishing = b.tick_idle(151_000, Dispatch::Ready, || 6).unwrap();
        assert!(matches!(&finishing.entries[..], [Entry::Idle { recent }] if recent.len() == 1));
        b.completed(target(Agent::Codex, "earlier").unwrap(), 151_001);
        assert!(b.update_idle(IdleGate::Ready, 151_001));
        assert!(!b.finish(finishing.id, None, 151_002));
        assert!(b
            .tick_idle(181_001, Dispatch::Ready, || panic!("completion owns idle"))
            .is_none());
    }

    fn final_text(b: &mut Banter, turn: &str, now: u64) {
        b.ingest(
            VisibleProgress {
                agent: Agent::Codex,
                session: "same-session".into(),
                turn: turn.into(),
                id: format!("final-{turn}"),
                text: "登录通过，保存还待验证。".into(),
                kind: BanterKind::Completion,
            },
            now,
        );
    }

    #[test]
    fn completion_bypasses_progress_interval_but_waits_for_done_and_expires() {
        let mut b = Banter::new("test".into(), true);
        progress(&mut b, Agent::Codex, "one", "draft", "checking", 0);
        let t = target(Agent::Codex, "one").unwrap();
        let work = b
            .tick(working(Agent::Codex, "one"), 0, Dispatch::Ready)
            .unwrap();
        final_text(&mut b, "one", 10);
        assert!(b.completed(t, 100));
        assert!(!b.finish(work.id, Some("还在检查。".into()), 110));
        let complete = b
            .tick_completion(CompletionGate::Waiting, 120, true)
            .0
            .unwrap();
        assert!(
            matches!(&complete.entries[..], [Entry::Final(text)] if text.contains("保存还待验证"))
        );
        assert!(b
            .tick_completion(CompletionGate::Waiting, 125, true)
            .0
            .is_none());
        assert!(b.finish(complete.id, Some("登录过啦，保存还待查。".into()), 130));
        b.tick_completion(CompletionGate::Idle, 3099, true);
        assert!(b.snapshot().text.is_none());
        b.tick_completion(CompletionGate::Idle, 3100, true);
        assert_eq!(b.snapshot().kind, Some(BanterKind::Completion));
        assert_eq!(b.snapshot().expires_at_ms, Some(11100));
        b.tick_completion(CompletionGate::Idle, 11100, true);
        assert!(b.snapshot().text.is_none());
        assert_eq!(b.completion_requests, 1);
        assert_eq!(b.completion_shown, 1);
    }

    #[test]
    fn completion_handles_missing_late_preempted_and_failed_results() {
        let mut b = Banter::new("test".into(), true);
        let t = target(Agent::Codex, "one").unwrap();
        b.completed(t.clone(), 0);
        assert!(b.accepts_final(Agent::Codex, "same-session", "one", 2000));
        assert!(!b.accepts_final(Agent::Codex, "same-session", "one", 2001));
        assert!(!b.accepts_final(Agent::Codex, "same-session", "other", 1));
        b.tick_completion(CompletionGate::Waiting, 2001, true);
        assert_eq!(b.completion_phase, "no-final");

        b.completed(t.clone(), 3000);
        final_text(&mut b, "one", 4500);
        assert!(b
            .tick_completion(CompletionGate::Idle, 6000, false)
            .0
            .is_none());
        let request = b
            .tick_completion(CompletionGate::Idle, 6100, true)
            .0
            .unwrap();
        assert!(b.tick_completion(CompletionGate::Blocked, 6200, true).1);
        assert!(!b.finish(request.id, Some("晚到的结果。".into()), 6300));

        b.completed(t.clone(), 7000);
        final_text(&mut b, "one", 7000);
        let request = b
            .tick_completion(CompletionGate::Waiting, 7000, true)
            .0
            .unwrap();
        b.finish(request.id, None, 7001);
        assert_eq!(b.completion_phase, "no-summary");
        assert!(b
            .tick_completion(CompletionGate::Idle, 10000, true)
            .0
            .is_none());

        b.completed(t, 11000);
        final_text(&mut b, "one", 11000);
        let request = b
            .tick_completion(CompletionGate::Waiting, 11000, true)
            .0
            .unwrap();
        assert!(b.tick_completion(CompletionGate::Idle, 31000, true).1);
        assert!(!b.finish(request.id, Some("已经迟到了。".into()), 31001));
        assert!(b.snapshot().text.is_none());
    }

    fn progress(b: &mut Banter, agent: Agent, turn: &str, id: &str, text: &str, now: u64) {
        b.ingest(
            VisibleProgress {
                agent,
                session: "same-session".into(),
                turn: turn.into(),
                id: id.into(),
                text: text.into(),
                kind: BanterKind::Progress,
            },
            now,
        );
    }
    fn target(agent: Agent, turn: &str) -> Option<Target> {
        Some(Target(agent, "same-session".into(), turn.into()))
    }
    fn working(agent: Agent, turn: &str) -> Option<(Target, QuietActivity)> {
        target(agent, turn).map(|target| (target, QuietActivity::Working))
    }

    #[test]
    fn prefix_is_append_only_and_switching_sources_invalidates_inflight_display() {
        let mut b = Banter::new("test".into(), true);
        let codex = working(Agent::Codex, "one");
        let claude = working(Agent::ClaudeCode, "one");
        progress(&mut b, Agent::Codex, "one", "a", "checking settings", 0);
        let first = b.tick(codex.clone(), 0, Dispatch::Ready).unwrap();
        assert!(matches!(&first.entries[..], [Entry::Progress(t)] if t == "checking settings"));
        b.finish(first.id, Some("还在检查设置呢。".into()), 20);
        assert_eq!(b.snapshot().expires_at_ms, Some(8020));
        b.tick(codex.clone(), 8020, Dispatch::Observe);
        assert_eq!(b.snapshot().text, None);
        progress(&mut b, Agent::Codex, "one", "a", "duplicate", 8050);
        progress(&mut b, Agent::Codex, "one", "b", "settings fixed", 8100);
        assert!(b.tick(codex.clone(), 29_999, Dispatch::Ready).is_none());
        let second = b.tick(codex, 30_000, Dispatch::Ready).unwrap();
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
        b.tick(claude.clone(), 30_050, Dispatch::Ready);
        b.finish(second.id, Some("设置修好啦。".into()), 30_100);
        assert_eq!(b.snapshot().text, None);
        let third = b.tick(claude, 60_000, Dispatch::Ready).unwrap();
        assert!(matches!(&third.entries[..], [Entry::Progress(t)] if t == "running tests"));
        b.reset(false);
        assert!(!b.finish(third.id, Some("这条不应出现。".into()), 60_100));
    }

    #[test]
    fn quiet_bubble_and_session_retention_follow_virtual_time() {
        let mut b = Banter::new("test".into(), true);
        let first = working(Agent::Codex, "one");
        b.tick(first.clone(), 0, Dispatch::Unavailable);
        b.tick(first.clone(), 119_999, Dispatch::Unavailable);
        assert!(b.snapshot().text.is_none());
        assert!(b.tick(first, 120_000, Dispatch::Unavailable).is_none());
        assert_eq!(b.snapshot().text.as_deref(), Some(QUIET_FALLBACK));
        assert!(b.tick(None, 120_001, Dispatch::Ready).is_none());
        assert!(b.snapshot().text.is_none());
        progress(&mut b, Agent::Codex, "one", "a", "test failed", 120_002);
        assert!(b
            .tick(working(Agent::Codex, "two"), 120_003, Dispatch::Ready)
            .is_none());
        b.tick(None, 720_002, Dispatch::Ready);
        assert!(b.segments.is_empty());
    }

    #[test]
    fn quiet_request_with_stale_signals_has_no_old_turn_context_and_displays_for_eight_seconds() {
        let mut b = Banter::new("test".into(), true);
        progress(&mut b, Agent::Codex, "old", "a", "以前的测试", 0);
        let work =
            target(Agent::Codex, "one").map(|target| (target, QuietActivity::AwaitingUpdate));
        b.tick(work.clone(), 0, Dispatch::Ready);
        assert!(b.tick(work.clone(), 119_999, Dispatch::Ready).is_none());
        assert!(b.tick(work.clone(), 120_000, Dispatch::Observe).is_none());
        assert!(b.snapshot().text.is_none());
        let request = b.tick(work.clone(), 120_000, Dispatch::Ready).unwrap();
        assert!(matches!(
            &request.entries[..],
            [Entry::Quiet {
                activity: QuietActivity::AwaitingUpdate,
                recent_progress: None,
                previous_line: None
            }]
        ));
        assert!(b.tick(work.clone(), 120_001, Dispatch::Ready).is_none());
        assert!(b.finish(request.id, Some("我陪你等消息呀。".into()), 120_010));
        assert_eq!(b.snapshot().text.as_deref(), Some("我陪你等消息呀。"));
        assert_eq!(b.snapshot().expires_at_ms, Some(128_010));
        b.tick(work.clone(), 128_010, Dispatch::Observe);
        assert!(b.snapshot().text.is_none());
        assert!(b.tick(work.clone(), 239_999, Dispatch::Ready).is_none());
        let next = b.tick(work, 240_000, Dispatch::Ready).unwrap();
        assert!(
            matches!(&next.entries[..], [Entry::Quiet { previous_line: Some(text), .. }]
            if text == "我陪你等消息呀。")
        );
        b.finish(next.id, Some("我陪你等消息呀。".into()), 240_012);
        assert!(b.snapshot().text.is_none());
    }

    #[test]
    fn quiet_uses_same_turn_progress_and_shares_the_interval_with_new_progress() {
        let mut b = Banter::new("test".into(), true);
        let work = working(Agent::ClaudeCode, "one");
        b.tick(work.clone(), 0, Dispatch::Ready);
        progress(&mut b, Agent::ClaudeCode, "one", "a", "测试已开始", 1_000);
        let first = b.tick(work.clone(), 1_000, Dispatch::Ready).unwrap();
        b.finish(first.id, Some("测试开始啦。".into()), 1_001);
        progress(&mut b, Agent::ClaudeCode, "one", "a", "重复消息", 119_000);
        assert!(b.tick(work.clone(), 120_999, Dispatch::Ready).is_none());
        let quiet = b.tick(work.clone(), 121_000, Dispatch::Ready).unwrap();
        assert!(matches!(&quiet.entries[..], [Entry::Quiet {
            activity: QuietActivity::Working, recent_progress: Some(text), ..
        }] if text == "测试已开始"));
        progress(&mut b, Agent::ClaudeCode, "one", "b", "测试已结束", 121_010);
        b.finish(quiet.id, Some("现在还在跑测试呢。".into()), 121_020);
        assert!(b.snapshot().text.is_none());
        assert!(b.tick(work.clone(), 150_999, Dispatch::Ready).is_none());
        let next = b.tick(work, 151_000, Dispatch::Ready).unwrap();
        assert!(
            matches!(&next.entries[..], [Entry::Progress(a), Entry::Summary(_), Entry::Progress(b)]
            if a == "测试已开始" && b == "测试已结束")
        );
    }

    #[test]
    fn quiet_result_uses_local_fallback_only_if_it_still_matches_current_work() {
        let mut b = Banter::new("test".into(), true);
        let work = working(Agent::Codex, "one");
        b.tick(work.clone(), 0, Dispatch::Ready);
        let failed = b.tick(work.clone(), 120_000, Dispatch::Ready).unwrap();
        b.finish(failed.id, None, 120_010);
        assert_eq!(b.snapshot().text.as_deref(), Some(QUIET_FALLBACK));
        let next = b.tick(work.clone(), 240_000, Dispatch::Ready).unwrap();
        let stale =
            target(Agent::Codex, "one").map(|target| (target, QuietActivity::AwaitingUpdate));
        b.tick(stale.clone(), 240_001, Dispatch::Observe);
        b.finish(next.id, Some("现在还在跑测试呢。".into()), 240_002);
        assert!(b.snapshot().text.is_none());
        let interrupted = b.tick(stale, 360_000, Dispatch::Ready).unwrap();
        b.tick(None, 360_001, Dispatch::Observe);
        b.finish(interrupted.id, None, 360_002);
        assert!(b.snapshot().text.is_none());
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
        let result = b
            .tick(working(Agent::Codex, "one"), 5, Dispatch::Ready)
            .unwrap();
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
