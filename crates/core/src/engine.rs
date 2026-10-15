use crate::adapter::{normalize, short_hash, CanonicalEvent, Kind};
use code_musume_contracts::*;
use std::collections::{BTreeMap, VecDeque};

pub const DONE_MS: u64 = 3_000;
pub const NOTIFICATION_MS: u64 = 4_000;
pub const ERROR_MS: u64 = 30_000;
pub const FADE_MS: u64 = 60_000;
pub const UNCERTAIN_MS: u64 = 300_000;
pub const SLEEP_MS: u64 = 600_000;
pub const MAX_SESSIONS: usize = 8;
type SessionKey = (Agent, String);
#[derive(Debug, Clone)]
struct Tool {
    turn: Option<String>,
    terminal: bool,
    name: Option<String>,
    fingerprint: Option<String>,
}
#[derive(Debug, Clone)]
struct Pending {
    turn: Option<String>,
    tool_id: Option<String>,
    name: Option<String>,
    fingerprint: Option<String>,
    since: u64,
    ambiguous: bool,
}
#[derive(Debug, Clone, Default)]
struct Actor {
    turn: Option<String>,
    active: bool,
    tools: BTreeMap<String, Tool>,
    pending: Vec<Pending>,
    closed_turns: VecDeque<String>,
    compacting: bool,
    last: u64,
}
#[derive(Debug, Clone)]
struct Notice {
    state: DisplayState,
    until: u64,
    key: String,
}
#[derive(Debug, Clone)]
struct Session {
    actors: BTreeMap<String, Actor>,
    notices: Vec<Notice>,
    last: u64,
    ended: bool,
    ambiguous: bool,
    background: usize,
    background_last: u64,
    degraded: bool,
}
impl Session {
    fn new(now: u64) -> Self {
        Self {
            actors: BTreeMap::new(),
            notices: vec![],
            last: now,
            ended: false,
            ambiguous: false,
            background: 0,
            background_last: now,
            degraded: false,
        }
    }
    fn pending(&self) -> usize {
        self.actors.values().map(|a| a.pending.len()).sum()
    }
    fn view(&self, now: u64) -> (DisplayState, Certainty, u64) {
        let pending_since = self
            .actors
            .values()
            .flat_map(|a| &a.pending)
            .map(|p| p.since)
            .min();
        if let Some(since) = pending_since {
            return (
                DisplayState::WaitingPermission,
                if now.saturating_sub(since) >= SLEEP_MS || self.ambiguous || self.degraded {
                    Certainty::Stale
                } else {
                    Certainty::Observed
                },
                self.last,
            );
        }
        if let Some(n) = self
            .notices
            .iter()
            .filter(|n| now < n.until)
            .max_by_key(|n| n.state.priority())
        {
            return (
                n.state,
                if self.degraded {
                    Certainty::Stale
                } else {
                    Certainty::Observed
                },
                self.last,
            );
        }
        let work_times: Vec<_> = self
            .actors
            .values()
            .filter(|a| a.tools.values().any(|t| !t.terminal))
            .map(|a| a.last)
            .chain((self.background > 0).then_some(self.background_last))
            .collect();
        let think_times: Vec<_> = self
            .actors
            .values()
            .filter(|a| a.active || a.compacting)
            .map(|a| a.last)
            .collect();
        let work_last = work_times
            .iter()
            .copied()
            .filter(|last| now.saturating_sub(*last) < UNCERTAIN_MS)
            .max();
        let think_last = think_times
            .iter()
            .copied()
            .filter(|last| now.saturating_sub(*last) < UNCERTAIN_MS)
            .max();
        let state = if work_last.is_some() {
            DisplayState::Working
        } else if think_last.is_some() {
            DisplayState::Thinking
        } else {
            DisplayState::Idle
        };
        let elapsed = now.saturating_sub(work_last.or(think_last).unwrap_or(self.last));
        let expired_activity =
            state == DisplayState::Idle && (!work_times.is_empty() || !think_times.is_empty());
        let certainty = if self.ambiguous || expired_activity {
            Certainty::Unknown
        } else if self.degraded || state != DisplayState::Idle && elapsed >= FADE_MS {
            Certainty::Stale
        } else {
            Certainty::Observed
        };
        (
            if certainty == Certainty::Unknown {
                DisplayState::Idle
            } else {
                state
            },
            certainty,
            self.last,
        )
    }
}

pub struct Engine {
    sessions: BTreeMap<SessionKey, Session>,
    seen: VecDeque<String>,
    dominant: Option<SessionKey>,
    projection: Projection,
    diagnostics: VecDeque<Diagnostic>,
    seq: u64,
    last_activity: u64,
    pub config: Config,
    missed: u64,
    pub character_forms: BTreeMap<String, String>,
    pub default_form: String,
}
impl Engine {
    pub fn new(instance_id: String, config: Config) -> Self {
        Self {
            sessions: BTreeMap::new(),
            seen: VecDeque::new(),
            dominant: None,
            diagnostics: VecDeque::new(),
            seq: 0,
            last_activity: 0,
            missed: 0,
            character_forms: BTreeMap::from([
                ("claude-code".into(), "claude".into()),
                ("codex".into(), "codex".into()),
            ]),
            default_form: "default".into(),
            projection: Projection {
                v: PROTOCOL_VERSION,
                instance_id,
                revision: 0,
                state: DisplayState::Idle,
                certainty: Certainty::Unknown,
                dominant_agent: None,
                session_label: None,
                pending_count: 0,
                active_sessions: 0,
                form_id: "default".into(),
                character_id: config.character_id.clone(),
                bubble_key: None,
                intensity: 0.25,
                missed_events: 0,
                sessions: vec![],
                capabilities: capabilities(),
            },
            config,
        }
    }
    pub fn snapshot(&self) -> Projection {
        self.projection.clone()
    }
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.iter().cloned().collect()
    }
    fn log(&mut self, agent: Option<Agent>, event: &str, session: Option<&str>, outcome: &str) {
        if self.diagnostics.len() >= 200 {
            self.diagnostics.pop_front();
        }
        self.diagnostics.push_back(Diagnostic {
            seq: self.seq,
            agent,
            event: event.chars().take(128).collect(),
            session_label: session.map(short_hash),
            outcome: outcome.into(),
        });
    }
    pub fn lost(&mut self, count: u64, now: u64) {
        self.missed = self.missed.saturating_add(count);
        for session in self.sessions.values_mut() {
            session.degraded = true;
        }
        self.log(None, "transport", None, "events-lost");
        self.tick(now);
    }
    pub fn reset(&mut self, now: u64) {
        self.sessions.clear();
        self.dominant = None;
        self.missed = 0;
        self.last_activity = now;
        self.tick(now);
    }
    pub fn ingest(&mut self, envelope: EnvelopeV2, now: u64) {
        self.seq += 1;
        if self.seen.contains(&envelope.id) {
            self.log(Some(envelope.agent), &envelope.event, None, "duplicate");
            return;
        }
        if self.seen.len() >= 4096 {
            self.seen.pop_front();
        }
        self.seen.push_back(envelope.id.clone());
        match normalize(&envelope) {
            Ok(Some(e)) => {
                let key = (e.agent, e.session.clone());
                if !self.sessions.contains_key(&key) && self.sessions.len() >= MAX_SESSIONS {
                    let victim = self
                        .sessions
                        .iter()
                        .filter(|(_, s)| {
                            s.pending() == 0 && (s.ended || s.view(now).0 == DisplayState::Idle)
                        })
                        .min_by_key(|(_, s)| (!s.ended, s.last))
                        .map(|(k, _)| k.clone());
                    if let Some(k) = victim {
                        self.sessions.remove(&k);
                    } else {
                        for session in self.sessions.values_mut() {
                            session.degraded = true;
                        }
                        self.log(
                            Some(e.agent),
                            &envelope.event,
                            Some(&e.session),
                            "session-capacity",
                        );
                        self.missed += 1;
                        self.tick(now);
                        return;
                    }
                }
                let s = self
                    .sessions
                    .entry(key)
                    .or_insert_with(|| Session::new(now));
                let outcome = Self::apply(s, &e, now);
                if outcome == "applied" {
                    self.last_activity = now;
                }
                self.log(Some(e.agent), &envelope.event, Some(&e.session), outcome);
            }
            Ok(None) => self.log(
                Some(envelope.agent),
                &envelope.event,
                None,
                "unsupported-or-ignored",
            ),
            Err(reason) => {
                self.missed += 1;
                for session in self.sessions.values_mut() {
                    session.degraded = true;
                }
                self.log(Some(envelope.agent), &envelope.event, None, reason);
            }
        }
        self.tick(now);
    }
    fn apply(s: &mut Session, e: &CanonicalEvent, now: u64) -> &'static str {
        if e.kind == Kind::SessionEnd {
            s.ended = true;
            s.actors.clear();
            s.notices.clear();
            s.last = now;
            s.background = 0;
            return "applied";
        }
        if e.kind == Kind::SessionStart {
            if s.ended {
                *s = Session::new(now);
            }
            // A delayed SessionStart never erases already-received work.
            s.ended = false;
            return "applied";
        }
        if s.ended {
            return "ended-session";
        }
        let actor_key = e.actor.as_deref().unwrap_or("@main").to_owned();
        if !s.actors.contains_key(&actor_key) && s.actors.len() >= 64 {
            s.ambiguous = true;
            return "actor-capacity";
        }
        let a = s.actors.entry(actor_key.clone()).or_default();
        let turn = e.turn.clone().or_else(|| a.turn.clone());
        if turn.as_ref().is_some_and(|t| a.closed_turns.contains(t)) {
            return "closed-turn";
        }
        let tool_key = e.tool_id.clone().unwrap_or_else(|| {
            format!(
                "{}:{}:{}",
                turn.as_deref().unwrap_or("?"),
                e.tool_name.as_deref().unwrap_or("?"),
                e.fingerprint.as_deref().unwrap_or("?")
            )
        });
        match e.kind {
            Kind::Prompt => {
                if a.turn == turn && a.active {
                    return "duplicate-prompt";
                }
                if let Some(old) = a.turn.take() {
                    if a.closed_turns.len() >= 64 {
                        a.closed_turns.pop_front();
                    }
                    a.closed_turns.push_back(old);
                }
                a.tools.clear();
                a.pending.clear();
                a.turn = e.turn.clone();
                a.active = true;
                a.compacting = false;
                s.notices.retain(|n| n.state != DisplayState::Done);
                s.ambiguous = false;
                s.degraded = false;
            }
            Kind::ToolStart => {
                if a.tools.get(&tool_key).is_some_and(|t| t.terminal) {
                    return "terminal-tool";
                }
                if a.tools.len() >= 256 {
                    s.ambiguous = true;
                    return "tool-capacity";
                }
                if a.turn.is_none() {
                    a.turn = turn.clone();
                }
                a.active = true;
                a.tools.insert(
                    tool_key,
                    Tool {
                        turn: turn.clone(),
                        terminal: false,
                        name: e.tool_name.clone(),
                        fingerprint: e.fingerprint.clone(),
                    },
                );
                for p in &mut a.pending {
                    if p.tool_id.is_none()
                        && p.turn == turn
                        && p.fingerprint.is_some()
                        && a.tools
                            .values()
                            .filter(|t| {
                                !t.terminal
                                    && t.turn == turn
                                    && t.name == p.name
                                    && t.fingerprint == p.fingerprint
                            })
                            .count()
                            > 1
                    {
                        p.ambiguous = true;
                        s.ambiguous = true;
                    }
                }
            }
            Kind::ToolEnd | Kind::ToolFailed | Kind::Denied => {
                if a.tools.get(&tool_key).is_some_and(|t| t.terminal) {
                    return "terminal-tool";
                }
                let candidates = a
                    .tools
                    .values()
                    .filter(|t| {
                        !t.terminal
                            && t.turn == turn
                            && t.name == e.tool_name
                            && t.fingerprint == e.fingerprint
                    })
                    .count();
                if a.tools.len() < 256 {
                    a.tools.insert(
                        tool_key.clone(),
                        Tool {
                            turn: turn.clone(),
                            terminal: true,
                            name: e.tool_name.clone(),
                            fingerprint: e.fingerprint.clone(),
                        },
                    );
                }
                // Anonymous events only clear anonymous requests, never another actor's.
                let matches: Vec<usize> = a
                    .pending
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        p.turn == turn
                            && match (&p.tool_id, &e.tool_id) {
                                (Some(a), Some(b)) => a == b,
                                (Some(_), None) => false,
                                (None, _) => {
                                    !p.ambiguous
                                        && candidates <= 1
                                        && p.fingerprint.is_some()
                                        && p.fingerprint == e.fingerprint
                                        && p.name == e.tool_name
                                }
                            }
                    })
                    .map(|(i, _)| i)
                    .collect();
                if matches.len() == 1 {
                    a.pending.remove(matches[0]);
                }
                if a.turn.is_none() {
                    a.turn = turn;
                }
                if matches!(e.kind, Kind::ToolFailed | Kind::Denied) {
                    let key = format!("error:{actor_key}:{tool_key}");
                    if !s.notices.iter().any(|n| n.key == key) {
                        if s.notices.len() >= 64 {
                            s.notices.remove(0);
                        }
                        s.notices.push(Notice {
                            state: DisplayState::Error,
                            until: now + ERROR_MS,
                            key,
                        });
                    }
                }
            }
            Kind::Permission => {
                if e.tool_id.is_some()
                    && a.tools
                        .get(&tool_key)
                        .is_some_and(|t| t.terminal && t.turn == turn)
                {
                    return "terminal-tool";
                }
                if a.pending.iter().any(|p| {
                    p.turn == turn
                        && p.tool_id == e.tool_id
                        && p.name == e.tool_name
                        && p.fingerprint == e.fingerprint
                }) {
                    return "duplicate-permission";
                }
                if a.pending.len() >= 64 {
                    s.ambiguous = true;
                    return "permission-capacity";
                }
                let ambiguous = e.tool_id.is_none()
                    && a.tools
                        .values()
                        .filter(|t| {
                            !t.terminal
                                && t.turn == turn
                                && t.name == e.tool_name
                                && t.fingerprint == e.fingerprint
                        })
                        .count()
                        > 1;
                if ambiguous {
                    s.ambiguous = true;
                }
                a.pending.push(Pending {
                    turn,
                    tool_id: e.tool_id.clone(),
                    name: e.tool_name.clone(),
                    fingerprint: e.fingerprint.clone(),
                    since: now,
                    ambiguous,
                });
            }
            Kind::Done | Kind::Error | Kind::Interrupt | Kind::ActorEnd => {
                if a.turn.is_some() && e.turn.is_some() && a.turn != e.turn {
                    return "other-turn";
                }
                if let Some(t) = turn.clone() {
                    if a.closed_turns.len() >= 64 {
                        a.closed_turns.pop_front();
                    }
                    a.closed_turns.push_back(t);
                }
                a.tools.retain(|_, t| t.turn != turn);
                a.pending.retain(|p| p.turn != turn);
                a.active = false;
                a.compacting = false;
                a.turn = None;
                if e.kind == Kind::Done {
                    s.background = e.background_count;
                    s.background_last = now;
                }
                if matches!(e.kind, Kind::Done | Kind::Error) {
                    let state = if e.kind == Kind::Done {
                        DisplayState::Done
                    } else {
                        DisplayState::Error
                    };
                    let key = format!(
                        "{}:{}:{}",
                        state.key(),
                        actor_key,
                        turn.as_deref().unwrap_or("?")
                    );
                    if !s.notices.iter().any(|n| n.key == key) {
                        if s.notices.len() >= 64 {
                            s.notices.remove(0);
                        }
                        s.notices.push(Notice {
                            state,
                            until: now
                                + if state == DisplayState::Done {
                                    DONE_MS
                                } else {
                                    ERROR_MS
                                },
                            key,
                        });
                    }
                }
            }
            Kind::Notification => {
                let key = format!("notification:{}", turn.as_deref().unwrap_or("?"));
                if s.notices.iter().any(|n| n.key == key) {
                    return "duplicate-notification";
                }
                if s.notices.len() >= 64 {
                    s.notices.remove(0);
                }
                s.notices.push(Notice {
                    state: DisplayState::Notification,
                    until: now + NOTIFICATION_MS,
                    key,
                });
            }
            Kind::ActorStart => {
                a.active = true;
                a.turn = turn;
            }
            Kind::CompactStart => a.compacting = true,
            Kind::CompactEnd => a.compacting = false,
            Kind::SessionStart | Kind::SessionEnd => unreachable!(),
        }
        s.last = now;
        a.last = now;
        "applied"
    }
    pub fn tick(&mut self, now: u64) -> bool {
        let summaries: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| !s.ended)
            .map(|(k, s)| {
                let (state, certainty, _) = s.view(now);
                SessionSummary {
                    label: short_hash(&k.1),
                    agent: k.0,
                    state,
                    certainty,
                    pending_count: s.pending() as u32,
                }
            })
            .collect();
        let best_priority = self
            .sessions
            .values()
            .filter(|s| !s.ended)
            .map(|s| s.view(now).0.priority())
            .max();
        let current = self
            .dominant
            .as_ref()
            .filter(|k| {
                self.sessions.get(*k).is_some_and(|s| {
                    !s.ended
                        && s.view(now).1 != Certainty::Unknown
                        && Some(s.view(now).0.priority()) == best_priority
                })
            })
            .cloned();
        let winner = current.or_else(|| {
            self.sessions
                .iter()
                .filter(|(_, s)| !s.ended)
                .max_by_key(|(_, s)| (s.view(now).0.priority(), s.last))
                .map(|(k, _)| k.clone())
        });
        let (state, certainty, agent, label, elapsed) = if let Some(k) = &winner {
            let s = &self.sessions[k];
            let (state, certainty, _) = s.view(now);
            (
                state,
                certainty,
                Some(k.0),
                Some(short_hash(&k.1)),
                now.saturating_sub(s.last),
            )
        } else {
            (DisplayState::Idle, Certainty::Unknown, None, None, 0)
        };
        let quiet = now.saturating_sub(self.last_activity);
        let state = if state == DisplayState::Idle
            && quiet >= SLEEP_MS
            && self
                .sessions
                .values()
                .all(|s| s.ended || (s.pending() == 0 && s.view(now).0 == DisplayState::Idle))
        {
            DisplayState::Sleeping
        } else {
            state
        };
        let bubble_key = match state {
            DisplayState::WaitingPermission
            | DisplayState::Error
            | DisplayState::Done
            | DisplayState::Notification => Some(state.key().to_owned()),
            _ => {
                if certainty == Certainty::Unknown && agent.is_some() {
                    Some("uncertain".into())
                } else {
                    None
                }
            }
        };
        let intensity = if certainty != Certainty::Observed {
            0.25
        } else {
            match state {
                DisplayState::WaitingPermission => {
                    if elapsed >= SLEEP_MS {
                        0.3
                    } else {
                        1.0
                    }
                }
                DisplayState::Error => 0.85,
                DisplayState::Done => 0.55,
                DisplayState::Notification => 0.5,
                DisplayState::Working => 0.5,
                _ => 0.25,
            }
        };
        let form_id = self
            .config
            .form_lock
            .clone()
            .or_else(|| agent.and_then(|a| self.config.agent_forms.get(a.as_str()).cloned()))
            .or_else(|| agent.and_then(|a| self.character_forms.get(a.as_str()).cloned()))
            .unwrap_or_else(|| self.default_form.clone());
        let mut p = Projection {
            v: PROTOCOL_VERSION,
            instance_id: self.projection.instance_id.clone(),
            revision: self.projection.revision,
            state,
            certainty,
            dominant_agent: agent,
            session_label: label,
            pending_count: self.sessions.values().map(Session::pending).sum::<usize>() as u32,
            active_sessions: summaries.len() as u32,
            form_id,
            character_id: self.config.character_id.clone(),
            bubble_key,
            intensity,
            missed_events: self.missed,
            sessions: summaries,
            capabilities: capabilities(),
        };
        self.dominant = winner;
        if p == self.projection {
            return false;
        }
        p.revision += 1;
        self.projection = p;
        true
    }
}
