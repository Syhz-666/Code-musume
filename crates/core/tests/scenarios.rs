use code_musume_contracts::*;
use code_musume_core::Engine;
use serde_json::{json, Value};
fn event(
    agent: Agent,
    name: &str,
    session: &str,
    turn: &str,
    extra: Value,
    id: &str,
) -> EnvelopeV2 {
    let mut p = json!({"session_id":session});
    p[if agent == Agent::ClaudeCode {
        "prompt_id"
    } else {
        "turn_id"
    }] = json!(turn);
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    EnvelopeV2 {
        v: 2,
        id: id.into(),
        agent,
        event: name.into(),
        ts: 0,
        src: EnvelopeSrc {
            cli: "test".into(),
            pid: 1,
        },
        payload: p.to_string(),
        parse_error: None,
    }
}
fn engine() -> Engine {
    Engine::new("test".into(), Config::default())
}
#[test]
fn two_agents_done_returns_to_other_work() {
    let mut e = engine();
    e.ingest(
        event(
            Agent::ClaudeCode,
            "PreToolUse",
            "same",
            "a",
            json!({"tool_use_id":"1"}),
            "1",
        ),
        0,
    );
    e.ingest(
        event(
            Agent::Codex,
            "PreToolUse",
            "same",
            "b",
            json!({"tool_use_id":"2"}),
            "2",
        ),
        1,
    );
    assert_eq!(e.snapshot().dominant_agent, Some(Agent::ClaudeCode));
    e.ingest(
        event(Agent::ClaudeCode, "Stop", "same", "a", json!({}), "3"),
        2,
    );
    assert_eq!(e.snapshot().state, DisplayState::Done);
    e.tick(3003);
    assert_eq!(e.snapshot().state, DisplayState::Working);
    assert_eq!(e.snapshot().form_id, "codex");
    assert_eq!(e.snapshot().active_sessions, 2);
}
#[test]
fn pending_is_not_cleared_by_other_tool_or_actor() {
    let mut e = engine();
    let a = Agent::Codex;
    e.ingest(
        event(
            a,
            "PermissionRequest",
            "s",
            "t",
            json!({"tool_name":"Bash","tool_input":{"command":"one"}}),
            "1",
        ),
        0,
    );
    e.ingest(
        event(
            a,
            "PostToolUse",
            "s",
            "t",
            json!({"tool_name":"Bash","tool_input":{"command":"two"},"tool_use_id":"2"}),
            "2",
        ),
        1,
    );
    assert_eq!(e.snapshot().pending_count, 1);
    e.ingest(
        event(
            a,
            "PostToolUse",
            "s",
            "t",
            json!({"agent_id":"child","tool_name":"Bash","tool_input":{"command":"one"}}),
            "3",
        ),
        2,
    );
    assert_eq!(e.snapshot().pending_count, 1);
    e.ingest(
        event(
            a,
            "PostToolUse",
            "s",
            "t",
            json!({"tool_name":"Bash","tool_input":{"command":"one"},"tool_use_id":"1"}),
            "4",
        ),
        3,
    );
    assert_eq!(e.snapshot().pending_count, 0);
}
#[test]
fn stop_and_interrupt_clear_only_their_turn() {
    for name in ["Stop", "Interrupt"] {
        let mut e = engine();
        e.ingest(
            event(Agent::Codex, "PermissionRequest", "s", "t", json!({}), "1"),
            0,
        );
        e.ingest(
            event(
                Agent::ClaudeCode,
                "PermissionRequest",
                "s",
                "t",
                json!({}),
                "2",
            ),
            1,
        );
        e.ingest(event(Agent::Codex, name, "s", "t", json!({}), "3"), 2);
        assert_eq!(e.snapshot().pending_count, 1);
        assert_eq!(e.snapshot().dominant_agent, Some(Agent::ClaudeCode));
    }
}
#[test]
fn finish_before_start_and_late_turn_cannot_reopen() {
    let mut e = engine();
    let a = Agent::Codex;
    e.ingest(
        event(
            a,
            "PostToolUse",
            "s",
            "t",
            json!({"tool_use_id":"tool"}),
            "1",
        ),
        0,
    );
    e.ingest(
        event(
            a,
            "PreToolUse",
            "s",
            "t",
            json!({"tool_use_id":"tool"}),
            "2",
        ),
        1,
    );
    assert_ne!(e.snapshot().state, DisplayState::Working);
    e.ingest(event(a, "Stop", "s", "t", json!({}), "3"), 2);
    e.ingest(
        event(
            a,
            "PreToolUse",
            "s",
            "t",
            json!({"tool_use_id":"late"}),
            "4",
        ),
        3,
    );
    e.tick(4000);
    assert_eq!(e.snapshot().state, DisplayState::Idle);
}
#[test]
fn prompt_supersedes_previous_notice_immediately() {
    let mut e = engine();
    let a = Agent::ClaudeCode;
    e.ingest(event(a, "Stop", "s", "old", json!({}), "1"), 0);
    e.ingest(event(a, "UserPromptSubmit", "s", "new", json!({}), "2"), 1);
    assert_eq!(e.snapshot().state, DisplayState::Thinking);
    e.ingest(event(a, "Stop", "s", "old", json!({}), "3"), 2);
    assert_eq!(e.snapshot().state, DisplayState::Thinking);
}
#[test]
fn duplicates_do_not_extend_notice_and_notification_preserves_work() {
    let mut e = engine();
    let a = Agent::ClaudeCode;
    e.ingest(
        event(a, "PreToolUse", "s", "t", json!({"tool_use_id":"x"}), "1"),
        0,
    );
    e.ingest(
        event(
            a,
            "Notification",
            "s",
            "t",
            json!({"notification_type":"idle_prompt"}),
            "2",
        ),
        1,
    );
    e.ingest(
        event(
            a,
            "Notification",
            "s",
            "t",
            json!({"notification_type":"idle_prompt"}),
            "3",
        ),
        2000,
    );
    e.tick(4002);
    assert_eq!(e.snapshot().state, DisplayState::Working);
}
#[test]
fn native_time_drives_hidden_window_state_and_stale_wait_never_vanishes() {
    let mut e = engine();
    e.ingest(
        event(Agent::Codex, "PreToolUse", "s", "t", json!({}), "1"),
        0,
    );
    e.tick(60001);
    assert_eq!(e.snapshot().certainty, Certainty::Stale);
    e.tick(300001);
    assert_eq!(e.snapshot().certainty, Certainty::Unknown);
    e.ingest(
        event(Agent::Codex, "PermissionRequest", "s", "t", json!({}), "2"),
        400000,
    );
    e.tick(1_500_000);
    assert_eq!(e.snapshot().state, DisplayState::WaitingPermission);
}
#[test]
fn malformed_data_never_creates_phantom_session_or_leaks_content() {
    let mut e = engine();
    let mut x = event(Agent::Codex, "Stop", "s", "t", json!({}), "1");
    x.payload = "SECRET prompt".into();
    e.ingest(x, 0);
    assert_eq!(e.snapshot().active_sessions, 0);
    assert!(!serde_json::to_string(&e.diagnostics())
        .unwrap()
        .contains("SECRET"));
}
#[test]
fn all_waiting_sessions_are_protected_from_eviction() {
    let mut e = engine();
    for i in 0..9 {
        e.ingest(
            event(
                Agent::Codex,
                "PermissionRequest",
                &i.to_string(),
                "t",
                json!({}),
                &i.to_string(),
            ),
            i,
        );
    }
    assert_eq!(e.snapshot().active_sessions, 8);
    assert_eq!(e.snapshot().pending_count, 8);
    assert_eq!(e.snapshot().missed_events, 1);
}
#[test]
fn manual_form_lock_wins_and_wall_clock_is_not_used_for_expiry() {
    let mut e = engine();
    e.config.form_lock = Some("custom".into());
    let mut x = event(Agent::Codex, "Stop", "s", "t", json!({}), "1");
    x.ts = 8_000_000_000_000;
    e.ingest(x, 1);
    assert_eq!(e.snapshot().form_id, "custom");
    e.tick(3002);
    assert_eq!(e.snapshot().state, DisplayState::Idle);
}
