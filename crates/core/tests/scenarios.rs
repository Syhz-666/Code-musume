use code_musume_contracts::*;
use code_musume_core::engine::CodexApprovals;
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
fn idle_banter_preserves_work_and_lost_signal_and_overrides_confirmed_sleep() {
    let mut e = engine();
    assert!(e.idle_banter_available());
    e.set_idle_resting(Some(true), 0);
    assert_eq!(e.snapshot().state, DisplayState::Sleeping);
    for (agent, id) in [(Agent::Codex, "codex"), (Agent::ClaudeCode, "claude")] {
        e.ingest(event(agent, "UserPromptSubmit", id, "t", json!({}), id), 1);
        assert!(!e.idle_banter_available());
        assert_eq!(e.snapshot().state, DisplayState::Thinking);
    }
    e.ingest(
        event(
            Agent::ClaudeCode,
            "PermissionRequest",
            "claude",
            "t",
            json!({"tool_use_id":"test", "tool_name":"Bash"}),
            "permission",
        ),
        2,
    );
    assert_eq!(e.snapshot().state, DisplayState::WaitingPermission);
    assert!(!e.idle_banter_available());
    e.ingest(
        event(
            Agent::ClaudeCode,
            "SessionEnd",
            "claude",
            "t",
            json!({}),
            "end",
        ),
        3,
    );
    e.tick(300_002);
    assert_eq!(e.snapshot().bubble_key.as_deref(), Some("uncertain"));
    assert!(!e.idle_banter_available());
    e.reset(300_003);
    e.set_idle_resting(Some(false), 300_003);
    e.tick(900_004);
    assert_eq!(e.snapshot().state, DisplayState::Idle);
    e.set_idle_resting(None, 900_004);
    assert_eq!(e.snapshot().state, DisplayState::Sleeping);
}
#[test]
fn main_stop_is_attributed_once_and_existing_parallel_work_allows_a_summary() {
    use code_musume_core::banter::{CompletionGate, Target};
    let mut e = engine();
    let cx = Agent::Codex;
    let cc = Agent::ClaudeCode;
    e.ingest(
        event(cx, "UserPromptSubmit", "codex", "1", json!({}), "a"),
        0,
    );
    e.ingest(event(cx, "Stop", "codex", "old", json!({}), "b"), 1);
    assert!(e.take_completed_turn().is_none());
    e.ingest(
        event(cc, "UserPromptSubmit", "claude", "2", json!({}), "parallel"),
        2,
    );
    e.ingest(
        event(
            cc,
            "PreToolUse",
            "claude",
            "2",
            json!({"tool_use_id":"other-tool"}),
            "tool",
        ),
        2,
    );
    let (work, _) = e.banter_work(None, 2).unwrap();
    assert!(work == Target(cc, "claude".into(), "2".into()));
    e.ingest(
        event(cx, "Stop", "codex", "1", json!({"agent_id":"worker"}), "c"),
        2,
    );
    assert!(e.take_completed_turn().is_none());
    e.ingest(event(cx, "Stop", "codex", "1", json!({}), "d"), 3);
    let t = e.take_completed_turn().unwrap();
    assert!(t == Target(cx, "codex".into(), "1".into()));
    assert!(e.completion_gate(&t, 3) == CompletionGate::Waiting);
    assert!(e.banter_work(Some(&work), 3).is_some());
    e.ingest(event(cx, "Stop", "codex", "1", json!({}), "repeat"), 4);
    assert!(e.take_completed_turn().is_none());
    e.tick(3003);
    assert_eq!(e.snapshot().state, DisplayState::Working);
    assert_eq!(e.snapshot().dominant_agent, Some(cc));
    assert!(e.completion_gate(&t, 3003) == CompletionGate::Available);
    e.ingest(
        event(cc, "UserPromptSubmit", "claude", "new", json!({}), "e"),
        3004,
    );
    assert!(e.completion_gate(&t, 3004) == CompletionGate::Blocked);
    e.ingest(
        event(cc, "StopFailure", "claude", "new", json!({}), "f"),
        3005,
    );
    assert!(e.take_completed_turn().is_none());
    e.ingest(
        event(cc, "UserPromptSubmit", "claude", "3", json!({}), "g"),
        3006,
    );
    e.ingest(
        event(cc, "Stop", "claude", "3", json!({"prompt_id":null}), "h"),
        3007,
    );
    assert!(e.take_completed_turn().is_none());
}
fn oversized_codex_tool_result(id: &str) -> EnvelopeV2 {
    let mut input = event(Agent::Codex, "PostToolUse", "codex", "t", json!({}), id);
    input.payload = "{\"tool_response\":\"".into();
    input.parse_error = Some(ParseError {
        kind: "size".into(),
    });
    input
}

#[test]
fn matched_stop_and_final_produce_summary_after_oversized_tool_result() {
    use code_musume_core::banter::{Banter, CompletionGate};
    use code_musume_core::engine::DONE_MS;

    let mut e = engine();
    let mut banter = Banter::new("test".into(), true);
    let cx = Agent::Codex;
    e.ingest(
        event(
            Agent::ClaudeCode,
            "SessionStart",
            "claude",
            "other",
            json!({}),
            "other-session",
        ),
        0,
    );
    e.ingest(
        event(cx, "UserPromptSubmit", "codex", "t", json!({}), "prompt"),
        1,
    );
    e.ingest(
        event(
            cx,
            "PreToolUse",
            "codex",
            "t",
            json!({"tool_use_id": "large"}),
            "start",
        ),
        2,
    );
    e.ingest(oversized_codex_tool_result("lost"), 3);
    assert_eq!(
        e.latest_diagnostic().unwrap().outcome,
        "oversized-tool-result"
    );
    assert_eq!(e.snapshot().certainty, Certainty::Stale);
    assert_eq!(e.snapshot().missed_events, 1);
    assert!(e
        .snapshot()
        .sessions
        .iter()
        .any(|s| s.agent == Agent::ClaudeCode && s.certainty == Certainty::Observed));

    banter.ingest(
        VisibleProgress {
            agent: cx,
            session: "codex".into(),
            turn: "t".into(),
            id: "final".into(),
            text: "测试通过，发布还未验证。".into(),
            kind: BanterKind::Completion,
        },
        4,
    );
    e.ingest(event(cx, "Stop", "codex", "t", json!({}), "stop"), 5);
    let target = e
        .take_completed_turn()
        .expect("exact Stop should close the degraded turn");
    assert_eq!(e.snapshot().certainty, Certainty::Observed);
    assert_eq!(e.snapshot().missed_events, 1);
    banter.completed(target.clone(), 5);
    assert!(e.completion_gate(&target, 5) == CompletionGate::Waiting);
    let request = banter
        .tick_completion(e.completion_gate(&target, 5), 5, true)
        .0
        .unwrap();

    // A result arriving out of order cannot make an already closed tool live again.
    e.ingest(oversized_codex_tool_result("late-result"), 6);
    assert_eq!(
        banter.finish(request.id, Some("测试过啦，发布还待查。".into()), 7),
        Some("ready")
    );
    assert!(banter.snapshot().text.is_none());
    let after_done = 5 + DONE_MS;
    e.tick(after_done);
    assert!(e.completion_gate(&target, after_done) == CompletionGate::Available);
    banter.tick_completion(e.completion_gate(&target, after_done), after_done, true);
    assert_eq!(banter.snapshot().kind, Some(BanterKind::Completion));
}

#[test]
fn oversized_tool_result_cannot_recover_an_unspecified_loss_or_active_child() {
    use code_musume_core::banter::CompletionGate;
    let cx = Agent::Codex;
    let running = || {
        let mut e = engine();
        e.ingest(
            event(cx, "UserPromptSubmit", "codex", "t", json!({}), "prompt"),
            0,
        );
        e.ingest(
            event(
                cx,
                "PreToolUse",
                "codex",
                "t",
                json!({"tool_use_id": "large"}),
                "start",
            ),
            1,
        );
        e
    };
    let mut unknown_loss = running();
    unknown_loss.lost(1, 2);
    unknown_loss.ingest(oversized_codex_tool_result("lost"), 3);
    unknown_loss.ingest(event(cx, "Stop", "codex", "t", json!({}), "stop"), 4);
    assert!(unknown_loss.take_completed_turn().is_none());
    assert_eq!(unknown_loss.snapshot().certainty, Certainty::Stale);

    let mut e = running();
    e.ingest(oversized_codex_tool_result("lost"), 3);
    e.ingest(
        event(cx, "Stop", "codex", "wrong-turn", json!({}), "wrong-stop"),
        4,
    );
    assert!(e.take_completed_turn().is_none());
    assert_eq!(e.snapshot().certainty, Certainty::Stale);
    e.ingest(
        event(
            cx,
            "SubagentStart",
            "codex",
            "t",
            json!({"agent_id": "worker"}),
            "child",
        ),
        5,
    );
    e.ingest(event(cx, "Stop", "codex", "t", json!({}), "stop"), 6);
    let target = e.take_completed_turn().unwrap();
    assert!(e.completion_gate(&target, 6) == CompletionGate::Blocked);
}

#[test]
fn codex_desktop_approvals_do_not_clear_other_sources_or_create_sessions() {
    let mut e = engine().with_codex_approval_sync(true);
    let pending = CodexApprovals {
        manual: 1,
        human_input: 0,
        automatic: 0,
    };
    e.update_codex_approvals("untracked".into(), Some(pending), 0);
    assert_eq!(e.snapshot().active_sessions, 0);
    e.ingest(
        event(
            Agent::ClaudeCode,
            "PermissionRequest",
            "same",
            "a",
            json!({"tool_name":"Bash","tool_input":{"command":"fixture"}}),
            "1",
        ),
        1,
    );
    e.ingest(
        event(
            Agent::Codex,
            "UserPromptSubmit",
            "same",
            "b",
            json!({}),
            "2",
        ),
        2,
    );
    e.ingest(
        event(
            Agent::Codex,
            "PermissionRequest",
            "same",
            "b",
            json!({"tool_name":"Bash","tool_input":{"command":"fixture"}}),
            "3",
        ),
        3,
    );
    // Before any desktop snapshot the Codex hook request counts as fallback evidence.
    assert_eq!(e.snapshot().pending_count, 2);
    e.update_codex_approvals("same".into(), Some(pending), 4);
    // One Claude request plus the desktop's count for the Codex session.
    assert_eq!(e.snapshot().pending_count, 2);
    e.ingest(
        event(Agent::Codex, "PostToolUse", "same", "b", json!({}), "4"),
        5,
    );
    assert_eq!(e.snapshot().pending_count, 2);
    e.update_codex_approvals("same".into(), None, 6);
    // The snapshot at 4 superseded the earlier hook request, so it does not come back.
    assert_eq!(e.snapshot().pending_count, 1);
    assert_eq!(e.snapshot().dominant_agent, Some(Agent::ClaudeCode));
    e.ingest(
        event(Agent::Codex, "SessionEnd", "same", "b", json!({}), "5"),
        7,
    );
    assert!(e.codex_sessions().is_empty());
    e.update_codex_approvals("same".into(), Some(pending), 8);
    assert_eq!(e.snapshot().pending_count, 1);
}
#[test]
fn same_codex_session_counts_desktop_and_hook_once() {
    let mut e = engine().with_codex_approval_sync(true);
    let identity = json!({"tool_name":"Bash","tool_input":{"command":"fixture"}});
    e.ingest(
        event(Agent::Codex, "UserPromptSubmit", "s", "t", json!({}), "1"),
        0,
    );
    e.ingest(
        event(Agent::Codex, "PermissionRequest", "s", "t", identity, "2"),
        1,
    );
    assert_eq!(e.snapshot().pending_count, 1);
    let one = CodexApprovals {
        manual: 1,
        human_input: 0,
        automatic: 0,
    };
    e.update_codex_approvals("s".into(), Some(one), 2);
    assert_eq!(e.snapshot().pending_count, 1);
    e.update_codex_approvals("x".into(), Some(one), 3);
    assert_eq!(
        e.diagnostics().last().map(|d| d.outcome.as_str()),
        Some("unknown-session")
    );
}
#[test]
fn desktop_updates_are_ignored_without_sync() {
    let mut e = engine();
    e.ingest(
        event(Agent::Codex, "UserPromptSubmit", "s", "t", json!({}), "1"),
        0,
    );
    let one = CodexApprovals {
        manual: 1,
        human_input: 0,
        automatic: 0,
    };
    e.update_codex_approvals("s".into(), Some(one), 1);
    assert_eq!(e.snapshot().pending_count, 0);
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
            event(
                Agent::Codex,
                "PermissionRequest",
                "s",
                "t",
                json!({"tool_name":"Bash","tool_input":{"command":"fixture"}}),
                "1",
            ),
            0,
        );
        e.ingest(
            event(
                Agent::ClaudeCode,
                "PermissionRequest",
                "s",
                "t",
                json!({"tool_name":"Bash","tool_input":{"command":"fixture"}}),
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
        event(
            Agent::Codex,
            "PermissionRequest",
            "s",
            "t",
            json!({"tool_name":"Bash","tool_input":{"command":"fixture"}}),
            "2",
        ),
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
                json!({"tool_name":"Bash","tool_input":{"command":"fixture"}}),
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
