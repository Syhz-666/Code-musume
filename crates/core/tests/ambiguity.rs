use code_musume_contracts::*;
use code_musume_core::Engine;
fn e(name: &str, id: &str, payload: &str) -> EnvelopeV2 {
    EnvelopeV2 {
        v: 2,
        id: id.into(),
        agent: Agent::Codex,
        event: name.into(),
        ts: 0,
        src: EnvelopeSrc {
            cli: "test".into(),
            pid: 1,
        },
        payload: payload.into(),
        parse_error: None,
    }
}
#[test]
fn indistinguishable_parallel_permissions_remain_until_turn_end() {
    let mut engine = Engine::new("test".into(), Config::default());
    for (id, tool) in [("a", "one"), ("b", "two")] {
        engine.ingest(e("PreToolUse",id,&format!(r#"{{"session_id":"s","turn_id":"t","tool_use_id":"{tool}","tool_name":"Bash","tool_input":{{"command":"same"}}}}"#)),0);
    }
    engine.ingest(e("PermissionRequest","c",r#"{"session_id":"s","turn_id":"t","tool_name":"Bash","tool_input":{"command":"same"}}"#),1);
    engine.ingest(e("PostToolUse","d",r#"{"session_id":"s","turn_id":"t","tool_use_id":"one","tool_name":"Bash","tool_input":{"command":"same"}}"#),2);
    assert_eq!(engine.snapshot().state, DisplayState::WaitingPermission);
    engine.ingest(e("Stop", "e", r#"{"session_id":"s","turn_id":"t"}"#), 3);
    assert_eq!(engine.snapshot().pending_count, 0);
}
#[test]
fn session_start_after_work_does_not_reset_and_session_end_is_terminal() {
    let mut engine = Engine::new("test".into(), Config::default());
    engine.ingest(
        e(
            "PreToolUse",
            "a",
            r#"{"session_id":"s","turn_id":"t","tool_use_id":"one"}"#,
        ),
        1,
    );
    engine.ingest(
        e(
            "SessionStart",
            "b",
            r#"{"session_id":"s","source":"startup"}"#,
        ),
        2,
    );
    assert_eq!(engine.snapshot().state, DisplayState::Working);
    engine.ingest(e("SessionEnd", "c", r#"{"session_id":"s"}"#), 3);
    engine.ingest(
        e(
            "PreToolUse",
            "d",
            r#"{"session_id":"s","turn_id":"t","tool_use_id":"two"}"#,
        ),
        4,
    );
    assert_eq!(engine.snapshot().active_sessions, 0);
}

#[test]
fn explicit_failed_tool_is_transient_and_duplicate_does_not_extend() {
    let mut engine = Engine::new("test".into(), Config::default());
    let payload =
        r#"{"session_id":"s","turn_id":"t","tool_use_id":"one","tool_response":{"exit_code":1}}"#;
    engine.ingest(e("PostToolUse", "a", payload), 0);
    assert_eq!(engine.snapshot().state, DisplayState::Error);
    engine.ingest(e("PostToolUse", "b", payload), 20_000);
    engine.tick(30_001);
    assert_ne!(engine.snapshot().state, DisplayState::Error);
}

#[test]
fn description_is_part_of_generic_tool_input_identity() {
    let mut engine = Engine::new("test".into(), Config::default());
    engine.ingest(e("PermissionRequest", "a", r#"{"session_id":"s","turn_id":"t","tool_name":"mcp_update","tool_input":{"description":"first"}}"#), 0);
    engine.ingest(e("PostToolUse", "b", r#"{"session_id":"s","turn_id":"t","tool_name":"mcp_update","tool_input":{"description":"second"}}"#), 1);
    assert_eq!(engine.snapshot().pending_count, 1);
}

#[test]
fn late_permission_cannot_reopen_terminal_tool() {
    let mut engine = Engine::new("test".into(), Config::default());
    let payload = r#"{"session_id":"s","turn_id":"t","tool_use_id":"one"}"#;
    engine.ingest(e("PostToolUse", "a", payload), 0);
    engine.ingest(e("PermissionRequest", "b", payload), 1);
    assert_eq!(engine.snapshot().pending_count, 0);
}

#[test]
fn codex_bash_approval_description_does_not_match_a_different_command() {
    let mut engine = Engine::new("test".into(), Config::default());
    engine.ingest(e("PreToolUse", "start", r#"{"session_id":"s","turn_id":"t","tool_use_id":"one","tool_name":"Bash","tool_input":{"command":"first"}}"#), 0);
    engine.ingest(e("PermissionRequest", "permission", r#"{"session_id":"s","turn_id":"t","tool_name":"Bash","tool_input":{"command":"first","description":"approval reason"}}"#), 1);
    engine.ingest(e("PostToolUse", "other", r#"{"session_id":"s","turn_id":"t","tool_use_id":"two","tool_name":"Bash","tool_input":{"command":"different"}}"#), 2);
    assert_eq!(engine.snapshot().pending_count, 1);
    engine.ingest(e("PostToolUse", "finish", r#"{"session_id":"s","turn_id":"t","tool_use_id":"one","tool_name":"Bash","tool_input":{"command":"first"}}"#), 3);
    assert_eq!(engine.snapshot().pending_count, 0);
    assert_eq!(engine.snapshot().state, DisplayState::Thinking);
}

#[test]
fn claude_bash_description_is_not_treated_as_codex_approval_metadata() {
    let mut engine = Engine::new("test".into(), Config::default());
    let mut permission = e(
        "PermissionRequest",
        "permission",
        r#"{"session_id":"s","prompt_id":"t","tool_name":"Bash","tool_input":{"command":"same","description":"first"}}"#,
    );
    permission.agent = Agent::ClaudeCode;
    engine.ingest(permission, 0);
    let mut finish = e(
        "PostToolUse",
        "finish",
        r#"{"session_id":"s","prompt_id":"t","tool_name":"Bash","tool_input":{"command":"same","description":"different"}}"#,
    );
    finish.agent = Agent::ClaudeCode;
    engine.ingest(finish, 1);
    assert_eq!(engine.snapshot().pending_count, 1);
}

#[test]
fn new_turn_recovers_certainty_without_erasing_loss_diagnostics() {
    let mut engine = Engine::new("test".into(), Config::default());
    engine.ingest(
        e(
            "UserPromptSubmit",
            "a",
            r#"{"session_id":"s","turn_id":"t"}"#,
        ),
        0,
    );
    engine.lost(1, 1);
    assert_eq!(engine.snapshot().certainty, Certainty::Stale);
    engine.ingest(
        e(
            "UserPromptSubmit",
            "b",
            r#"{"session_id":"s","turn_id":"next"}"#,
        ),
        2,
    );
    assert_eq!(engine.snapshot().certainty, Certainty::Observed);
    assert_eq!(engine.snapshot().missed_events, 1);
}

#[test]
fn parent_completion_does_not_refresh_stale_child_activity() {
    let mut engine = Engine::new("test".into(), Config::default());
    engine.ingest(
        e(
            "PreToolUse",
            "child",
            r#"{"session_id":"s","turn_id":"child-turn","agent_id":"child","tool_use_id":"one"}"#,
        ),
        0,
    );
    engine.ingest(
        e(
            "Stop",
            "parent",
            r#"{"session_id":"s","turn_id":"parent-turn"}"#,
        ),
        300_001,
    );
    engine.tick(303_002);
    assert_eq!(engine.snapshot().state, DisplayState::Idle);
    assert_eq!(engine.snapshot().certainty, Certainty::Unknown);
}
