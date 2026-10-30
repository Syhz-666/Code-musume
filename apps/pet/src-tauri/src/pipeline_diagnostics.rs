use code_musume_contracts::{
    diagnostics::{safe_event, InputDiagnostics},
    *,
};
use code_musume_core::adapter::short_hash;
use serde::Serialize;
use std::collections::VecDeque;

pub const TRACE_CAPACITY: usize = 256;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSummary {
    revision: u64,
    state: DisplayState,
    certainty: Certainty,
    agent: Option<Agent>,
    pending: u32,
    missed: u64,
}
impl From<&Projection> for StateSummary {
    fn from(p: &Projection) -> Self {
        Self {
            revision: p.revision,
            state: p.state,
            certainty: p.certainty,
            agent: p.dominant_agent,
            pending: p.pending_count,
            missed: p.missed_events,
        }
    }
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineTrace {
    pub seq: u64,
    pub at_ms: u64,
    pub stage: &'static str,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<Agent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<InputDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<StateSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<StateSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribers: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
}
impl PipelineTrace {
    pub fn new(stage: &'static str, outcome: &str) -> Self {
        Self {
            stage,
            outcome: outcome.into(),
            ..Self::default()
        }
    }
    pub fn event(stage: &'static str, outcome: &str, e: &EnvelopeV2) -> Self {
        Self {
            event_id: Some(short_hash(&e.id)),
            event: Some(safe_event(&e.event).into()),
            agent: Some(e.agent),
            ..Self::new(stage, outcome)
        }
    }
}
#[derive(Default)]
pub struct PipelineDiagnostics {
    seq: u64,
    evicted: u64,
    entries: VecDeque<PipelineTrace>,
}
impl PipelineDiagnostics {
    pub fn record(&mut self, mut entry: PipelineTrace, at_ms: u64) {
        self.seq += 1;
        if self.entries.len() == TRACE_CAPACITY {
            self.entries.pop_front();
            self.evicted += 1;
        }
        entry.seq = self.seq;
        entry.at_ms = at_ms;
        self.entries.push_back(entry);
    }
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"capacity": TRACE_CAPACITY, "evicted": self.evicted, "entries": self.entries})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_is_bounded_and_unknown_event_names_and_ids_are_not_saved_verbatim() {
        let mut log = PipelineDiagnostics::default();
        let event = EnvelopeV2 {
            v: 2,
            id: "sensitive-id".into(),
            agent: Agent::Codex,
            event: "secret-command".into(),
            ts: 0,
            payload: "secret-prompt".into(),
            parse_error: None,
            src: EnvelopeSrc {
                cli: "test".into(),
                pid: 1,
            },
        };
        for n in 0..300 {
            log.record(PipelineTrace::event("http", "received", &event), n);
        }
        let snapshot = log.snapshot();
        assert_eq!(
            snapshot["entries"].as_array().unwrap().len(),
            TRACE_CAPACITY
        );
        assert_eq!(snapshot["evicted"], 44);
        let serialized = snapshot.to_string();
        for secret in ["sensitive-id", "secret-command", "secret-prompt"] {
            assert!(!serialized.contains(secret));
        }
    }
}
