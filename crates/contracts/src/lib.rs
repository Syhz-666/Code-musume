//! Shared wire contracts. Work semantics live exclusively in code-musume-core.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: u32 = 2;
pub const MAX_INPUT_BYTES: usize = 256 * 1024;
pub const MAX_ENVELOPE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_QUEUE_ENTRIES: usize = 128;
pub const MAX_QUEUE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub enum Agent {
    ClaudeCode,
    Codex,
}
impl Agent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "claude-code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct EnvelopeSrc {
    pub cli: String,
    pub pid: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct ParseError {
    pub kind: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct EnvelopeV2 {
    pub v: u32,
    pub id: String,
    pub agent: Agent,
    pub event: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub ts: u64,
    pub src: EnvelopeSrc,
    pub payload: String,
    pub parse_error: Option<ParseError>,
}
impl EnvelopeV2 {
    pub fn validate(&self) -> bool {
        self.v == PROTOCOL_VERSION
            && !self.id.is_empty()
            && self.id.len() <= 128
            && !self.event.is_empty()
            && self.event.len() <= 128
            && self.payload.len() <= MAX_INPUT_BYTES
            && self.ts <= 9_007_199_254_740_991
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub enum DisplayState {
    WaitingPermission,
    Error,
    Done,
    Notification,
    Working,
    Thinking,
    Idle,
    Sleeping,
}
impl DisplayState {
    pub fn priority(self) -> u8 {
        match self {
            Self::WaitingPermission => 100,
            Self::Error => 90,
            Self::Done => 80,
            Self::Notification => 70,
            Self::Working => 60,
            Self::Thinking => 50,
            Self::Idle => 20,
            Self::Sleeping => 10,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::WaitingPermission => "waiting-permission",
            Self::Error => "error",
            Self::Done => "done",
            Self::Notification => "notification",
            Self::Working => "working",
            Self::Thinking => "thinking",
            Self::Idle => "idle",
            Self::Sleeping => "sleeping",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub enum Certainty {
    Observed,
    Stale,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct AgentCapabilities {
    pub agent: Agent,
    pub turn_identity: String,
    pub interrupt: bool,
    pub turn_failure: bool,
    pub notification: bool,
    pub tool_actor_identity: bool,
}
pub fn capabilities() -> Vec<AgentCapabilities> {
    vec![
        AgentCapabilities {
            agent: Agent::ClaudeCode,
            turn_identity: "prompt_id".into(),
            interrupt: false,
            turn_failure: true,
            notification: true,
            tool_actor_identity: true,
        },
        AgentCapabilities {
            agent: Agent::Codex,
            turn_identity: "turn_id".into(),
            interrupt: true,
            turn_failure: false,
            notification: false,
            tool_actor_identity: false,
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct SessionSummary {
    pub label: String,
    pub agent: Agent,
    pub state: DisplayState,
    pub certainty: Certainty,
    pub pending_count: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Projection {
    pub v: u32,
    pub instance_id: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub revision: u64,
    pub state: DisplayState,
    pub certainty: Certainty,
    pub dominant_agent: Option<Agent>,
    pub session_label: Option<String>,
    pub pending_count: u32,
    pub active_sessions: u32,
    pub form_id: String,
    pub character_id: String,
    pub bubble_key: Option<String>,
    pub intensity: f32,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub missed_events: u64,
    pub sessions: Vec<SessionSummary>,
    pub capabilities: Vec<AgentCapabilities>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Position {
    pub monitor_id: String,
    pub x_phys: i32,
    pub y_phys: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Config {
    pub schema_version: u32,
    pub character_id: String,
    pub form_lock: Option<String>,
    pub agent_forms: BTreeMap<String, String>,
    pub ghost: bool,
    pub user_hidden: bool,
    pub reduced_motion: bool,
    pub fullscreen_hide: bool,
    pub autostart: bool,
    pub renderer: String,
    pub position: Option<Position>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            character_id: "builtin".into(),
            form_lock: None,
            agent_forms: BTreeMap::new(),
            ghost: false,
            user_hidden: false,
            reduced_motion: false,
            fullscreen_hide: true,
            autostart: false,
            renderer: "pixi".into(),
            position: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Diagnostic {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub seq: u64,
    pub agent: Option<Agent>,
    pub event: String,
    pub session_label: Option<String>,
    pub outcome: String,
}

/// Software-side acknowledgement after the pet has submitted the matching frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct RenderReceipt {
    pub instance_id: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub revision: u64,
    pub latency_ms: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFile {
    pub v: u32,
    pub port: u16,
    pub token: String,
    pub pid: u32,
    pub app_version: String,
    pub instance_id: String,
    pub started_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct Clip {
    pub row: u32,
    pub frames: u32,
    pub fps: f32,
    pub looping: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct CharacterForm {
    pub id: String,
    pub image: String,
    pub cell_width: u32,
    pub cell_height: u32,
    pub columns: u32,
    pub clips: BTreeMap<String, Clip>,
    pub look_rows: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct CharacterManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub default_form: String,
    pub agent_forms: BTreeMap<String, String>,
    pub forms: Vec<CharacterForm>,
    pub license: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct InstallPreview {
    pub agent: Agent,
    pub operation: String,
    pub target: String,
    pub digest: String,
    pub changes: String,
    pub warnings: Vec<String>,
    pub changed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct HookStatus {
    pub agent: Agent,
    pub target: String,
    pub installed: bool,
    pub needs_trust: bool,
    pub runtime: String,
}
