use code_musume_contracts::{Agent, EnvelopeV2};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    SessionStart,
    SessionEnd,
    Prompt,
    ToolStart,
    ToolEnd,
    ToolFailed,
    Permission,
    Denied,
    Notification,
    Done,
    Error,
    ActorStart,
    ActorEnd,
    CompactStart,
    CompactEnd,
    Interrupt,
}
#[derive(Debug, Clone)]
pub struct CanonicalEvent {
    pub agent: Agent,
    pub session: String,
    pub turn: Option<String>,
    pub actor: Option<String>,
    pub tool_id: Option<String>,
    pub tool_name: Option<String>,
    pub fingerprint: Option<String>,
    pub kind: Kind,
    pub background_count: usize,
}
fn field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 512)
        .map(str::to_owned)
}
fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let keys: std::collections::BTreeMap<_, _> = o.iter().collect();
            Value::Object(
                keys.into_iter()
                    .map(|(k, v)| (k.clone(), sorted(v)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        _ => v.clone(),
    }
}
pub fn short_hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))[..10].to_owned()
}
pub fn normalize(e: &EnvelopeV2) -> Result<Option<CanonicalEvent>, &'static str> {
    if !e.validate() {
        return Err("invalid-envelope");
    }
    if e.parse_error.is_some() {
        return Err("invalid-payload");
    }
    let v: Value = serde_json::from_str(&e.payload).map_err(|_| "invalid-json")?;
    let session = field(&v, "session_id").ok_or("missing-session")?;
    let kind = match e.event.as_str() {
        "SessionStart" => Kind::SessionStart,
        "SessionEnd" => Kind::SessionEnd,
        "UserPromptSubmit" => Kind::Prompt,
        "PreToolUse" => Kind::ToolStart,
        "PostToolUse" => {
            let response = &v["tool_response"];
            let failed = e.agent == Agent::Codex
                && (response.get("is_error").and_then(Value::as_bool) == Some(true)
                    || response.get("isError").and_then(Value::as_bool) == Some(true)
                    || response
                        .get("exit_code")
                        .or_else(|| response.get("exitCode"))
                        .and_then(Value::as_i64)
                        .is_some_and(|n| n != 0));
            if failed {
                Kind::ToolFailed
            } else {
                Kind::ToolEnd
            }
        }
        "PostToolUseFailure" if e.agent == Agent::ClaudeCode => Kind::ToolFailed,
        "PermissionRequest" => Kind::Permission,
        "PermissionDenied" if e.agent == Agent::ClaudeCode => Kind::Denied,
        "Stop" => Kind::Done,
        "StopFailure" if e.agent == Agent::ClaudeCode => Kind::Error,
        "Interrupt" if e.agent == Agent::Codex => Kind::Interrupt,
        "SubagentStart" => Kind::ActorStart,
        "SubagentStop" => Kind::ActorEnd,
        "PreCompact" => Kind::CompactStart,
        "PostCompact" => Kind::CompactEnd,
        "Notification" if e.agent == Agent::ClaudeCode => match v["notification_type"].as_str() {
            Some("permission_prompt" | "agent_needs_input") => Kind::Permission,
            Some("idle_prompt") => Kind::Notification,
            Some("agent_completed") => Kind::Done,
            Some(_) => return Ok(None),
            None => return Err("missing-notification-type"),
        },
        _ => return Ok(None),
    };
    let turn = field(
        &v,
        if e.agent == Agent::ClaudeCode {
            "prompt_id"
        } else {
            "turn_id"
        },
    );
    let actor = field(&v, "agent_id");
    if matches!(kind, Kind::ActorStart | Kind::ActorEnd) && actor.is_none() {
        return Err("missing-actor");
    }
    let fingerprint = v.get("tool_input").map(|input| {
        let mut identity = input.clone();
        // Codex Bash adds an approval-only description absent from Pre/PostToolUse.
        // Other tools can use description as real input, so keep their identity strict.
        if e.agent == Agent::Codex
            && v["tool_name"].as_str() == Some("Bash")
            && input.get("command").is_some_and(Value::is_string)
        {
            if let Some(fields) = identity.as_object_mut() {
                fields.remove("description");
            }
        }
        format!(
            "{:x}",
            Sha256::digest(sorted(&identity).to_string().as_bytes())
        )
    });
    Ok(Some(CanonicalEvent {
        agent: e.agent,
        session,
        turn,
        actor,
        tool_id: field(&v, "tool_use_id"),
        tool_name: field(&v, "tool_name"),
        fingerprint,
        kind,
        background_count: v["background_tasks"].as_array().map_or(0, Vec::len),
    }))
}
