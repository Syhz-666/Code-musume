//! Opt-in, one-file-per-invocation reports. Hook stdout/stderr remain empty.
use code_musume_contracts::{
    diagnostics::{safe_event, InputDiagnostics},
    Agent,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::Path, time::Instant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookReport {
    pub v: u32,
    pub trace_id: String,
    pub cli_version: &'static str,
    pub budget_ms: u64,
    pub agent: Option<Agent>,
    pub event: String,
    pub input: Option<InputDiagnostics>,
    pub outcome: &'static str,
    pub http_status: Option<u16>,
    pub elapsed_ms: f64,
    stages: Vec<Stage>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Stage {
    stage: &'static str,
    at_ms: f64,
}
impl HookReport {
    pub fn new(id: &str, agent: Option<Agent>, event: &str, budget_ms: u64) -> Self {
        Self {
            v: 1,
            trace_id: format!("{:x}", Sha256::digest(id.as_bytes()))[..10].into(),
            cli_version: env!("CARGO_PKG_VERSION"),
            budget_ms,
            agent,
            event: safe_event(event).into(),
            input: None,
            outcome: "not-completed",
            http_status: None,
            elapsed_ms: 0.0,
            stages: vec![],
        }
    }
    pub fn stage(&mut self, stage: &'static str, start: Instant) {
        if self.stages.len() < 8 {
            self.stages.push(Stage {
                stage,
                at_ms: start.elapsed().as_secs_f64() * 1000.0,
            });
        }
    }
    pub fn save(&mut self, path: &Path, start: Instant) {
        self.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        let Ok(bytes) = serde_json::to_vec(self) else {
            return;
        };
        if bytes.len() > 4096 {
            return;
        }
        // Never overwrite an existing user file; callers allocate a fresh path.
        if let Ok(mut file) = OpenOptions::new().write(true).create_new(true).open(path) {
            let _ = file.write_all(&bytes);
        }
    }
}
