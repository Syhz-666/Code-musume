use super::Agent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub enum BanterProtocol {
    #[default]
    OpenAi,
    Anthropic,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct BanterConfig {
    pub enabled: bool,
    pub protocol: BanterProtocol,
    pub base_url: String,
    pub model: String,
    pub nickname: String,
    pub prompt_cache: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleProgress {
    pub agent: Agent,
    pub session: String,
    pub turn: String,
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct BanterProjection {
    pub instance_id: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub revision: u64,
    pub agent: Option<Agent>,
    pub session_label: Option<String>,
    pub text: Option<String>,
    #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
    pub expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct BanterUsage {
    pub cache_read: Option<u32>,
    pub cache_write: Option<u32>,
    pub ordinary_input: Option<u32>,
    pub total_input: Option<u32>,
    pub output: Option<u32>,
    pub elapsed_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub struct BanterStatus {
    pub config: BanterConfig,
    pub has_key: bool,
    pub codex_ready: bool,
    pub claude_ready: bool,
    pub outcome: String,
    pub skipped_long: u32,
    pub usage: Option<BanterUsage>,
}
