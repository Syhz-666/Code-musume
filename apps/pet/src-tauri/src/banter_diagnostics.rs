use code_musume_contracts::{Agent, BanterKind, BanterProjection};
use code_musume_core::{adapter::short_hash, banter::Target};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};

const CAPACITY: usize = 128;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    at_ms: u64,
    kind: &'static str,
    step: &'static str,
    reason: &'static str,
    agent: Option<Agent>,
    session_label: Option<String>,
    turn_label: Option<String>,
    request_id: Option<u64>,
    revision: Option<u64>,
    elapsed_ms: Option<u64>,
    provider_error: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_rejection: Option<&'static str>,
}

impl Trace {
    pub fn new(kind: &'static str, step: &'static str, reason: &'static str) -> Self {
        Self {
            at_ms: 0,
            kind,
            step,
            reason,
            agent: None,
            session_label: None,
            turn_label: None,
            request_id: None,
            revision: None,
            elapsed_ms: None,
            provider_error: None,
            output_rejection: None,
        }
    }

    pub fn conversation(mut self, agent: Agent, session: &str, turn: &str) -> Self {
        self.agent = Some(agent);
        self.session_label = Some(short_hash(session));
        self.turn_label = Some(short_hash(turn));
        self
    }

    pub fn request(id: u64, kind: &'static str, target: Option<&Target>) -> Self {
        let mut trace = Self::new(kind, "request", "started");
        trace.request_id = Some(id);
        if let Some(Target(agent, session, turn)) = target {
            trace = trace.conversation(*agent, session, turn);
        }
        trace
    }

    pub fn outcome(mut self, step: &'static str, reason: &'static str) -> Self {
        self.step = step;
        self.reason = reason;
        self
    }

    pub fn elapsed(mut self, elapsed_ms: u64) -> Self {
        self.elapsed_ms = Some(elapsed_ms);
        self
    }

    pub fn revision(mut self, revision: u64) -> Self {
        self.revision = Some(revision);
        self
    }

    pub fn provider_error(mut self, error: Option<&'static str>) -> Self {
        self.provider_error = error;
        self
    }

    pub fn output_rejection(mut self, reason: Option<&'static str>) -> Self {
        self.output_rejection = reason;
        self
    }
}

pub fn kind(kind: BanterKind) -> &'static str {
    match kind {
        BanterKind::Progress => "progress",
        BanterKind::Completion => "completion",
        BanterKind::Idle => "idle",
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Counts {
    accepted_messages: u64,
    skipped_messages: u64,
    model_requests: u64,
    model_responses: u64,
    cancelled_requests: u64,
    published_bubbles: u64,
    rendered_bubbles: u64,
}

#[derive(Default)]
pub struct BanterDiagnostics {
    entries: VecDeque<Trace>,
    counts: BTreeMap<&'static str, Counts>,
    evicted: u64,
    last_rendered: Option<u64>,
}

impl BanterDiagnostics {
    pub fn record(&mut self, mut entry: Trace, now: u64) {
        let counts = self.counts.entry(entry.kind).or_default();
        match (entry.step, entry.reason) {
            ("source-message", "accepted") => counts.accepted_messages += 1,
            ("source-message", _) => counts.skipped_messages += 1,
            ("request", "started") => counts.model_requests += 1,
            ("result", _) => counts.model_responses += 1,
            ("request", "cancelled") => counts.cancelled_requests += 1,
            ("bubble", "published") => counts.published_bubbles += 1,
            ("bubble", "frame-submitted") => counts.rendered_bubbles += 1,
            _ => {}
        }
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
            self.evicted += 1;
        }
        entry.at_ms = now;
        self.entries.push_back(entry);
    }

    pub fn publication(&mut self, view: &BanterProjection, trace: Option<Trace>, now: u64) {
        if let Some(kind) = view.kind {
            let mut entry = trace
                .unwrap_or_else(|| Trace::new(self::kind(kind), "bubble", "published"))
                .outcome("bubble", "published")
                .revision(view.revision);
            entry.agent = view.agent;
            entry.session_label.clone_from(&view.session_label);
            self.record(entry, now);
        }
    }

    pub fn rendered(&mut self, revision: u64, now: u64) {
        if self.last_rendered.is_some_and(|last| last >= revision) {
            return;
        }
        if let Some(published) = self.entries.iter().rev().find(|entry| {
            entry.step == "bubble"
                && entry.reason == "published"
                && entry.revision == Some(revision)
        }) {
            let trace = published
                .clone()
                .outcome("bubble", "frame-submitted")
                .elapsed(now.saturating_sub(published.at_ms));
            self.last_rendered = Some(revision);
            self.record(trace, now);
        }
    }

    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "capacity": CAPACITY, "evicted": self.evicted, "counts": self.counts, "entries": self.entries
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_aggregate_counts_after_eviction_without_exposing_ids() {
        let mut log = BanterDiagnostics::default();
        for n in 0..=CAPACITY {
            log.record(
                Trace::new("progress", "source-message", "accepted").conversation(
                    Agent::Codex,
                    "private-session",
                    "private-turn",
                ),
                n as u64,
            );
        }
        let snapshot = log.snapshot();
        assert_eq!(
            snapshot["counts"]["progress"]["acceptedMessages"],
            CAPACITY + 1
        );
        assert_eq!(snapshot["entries"].as_array().unwrap().len(), CAPACITY);
        assert_eq!(snapshot["evicted"], 1);
        assert!(!snapshot.to_string().contains("private-"));
    }
}
