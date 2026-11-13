use super::*;
use code_musume_contracts::{Agent, Config, DisplayState, EnvelopeSrc, EnvelopeV2};
use code_musume_core::Engine;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

const MANUAL: &str = "item/commandExecution/requestApproval";

#[test]
fn captures_only_new_completed_visible_messages_from_local_patches() {
    let mut observer = Observer {
        capture: true,
        ..Default::default()
    };
    observer.streams.insert("local-thread".into(), None);
    let initial = json!({"requests":[],"turnHistory":{"history":{"entitiesByKey":{"current":{
        "turnId":"turn","status":"inProgress","items":[
            {"type":"agentMessage","id":"old","phase":"commentary","text":"old content","completedAtMs":1},
            {"type":"reasoning","id":"hidden","text":"private reasoning","completedAtMs":1},
            {"type":"agentMessage","id":"new","phase":"commentary","text":"first","completedAtMs":null}
        ]
    }}}}});
    let output = observer.frame(&snapshot("local-thread", 1, initial.clone()));
    assert!(output
        .iter()
        .all(|event| !matches!(event, Observation::Progress(_))));
    assert!(observer.frame(&delta("local-thread", 1, json!([
        {"op":"replace","path":["turnHistory","history","entitiesByKey","current","items",2,"text"],"value":"new progress"}
    ]))).is_empty());
    let complete = delta(
        "local-thread",
        2,
        json!([
            {"op":"replace","path":["turnHistory","history","entitiesByKey","current","items",2,"completedAtMs"],"value":2}
        ]),
    );
    let output = observer.frame(&complete);
    assert!(
        matches!(&output[..], [Observation::Progress(p)] if p.id == "new" && p.text == "new progress" && p.turn == "turn")
    );
    assert!(observer.frame(&complete).is_empty());
    assert!(observer
        .frame(&snapshot("local-thread", 4, initial))
        .is_empty());
    let stream = observer.streams["local-thread"].as_ref().unwrap();
    assert!(!stream.state.to_string().contains("first"));
    assert!(!stream
        .progress
        .as_ref()
        .unwrap()
        .to_string()
        .contains("private reasoning"));
}

fn frame(id: &str, change: Value) -> Value {
    json!({
        "type":"broadcast", "method":"thread-stream-state-changed", "version":11,
        "sourceClientId":"desktop",
        "params":{"hostId":"local", "conversationId":id, "change":change},
    })
}

fn snapshot(id: &str, revision: u64, state: Value) -> Value {
    frame(
        id,
        json!({"type":"snapshot","revision":revision,"conversationState":state}),
    )
}

fn delta(id: &str, base: u64, changes: Value) -> Value {
    frame(
        id,
        json!({"type":"patches","baseRevision":base,"revision":base+1,"patches":changes}),
    )
}

fn state(manual: usize, human_input: usize, automatic: usize) -> CodexApprovals {
    CodexApprovals {
        manual,
        human_input,
        automatic,
    }
}

fn assert_change(observed: Vec<Observation>, expected: Option<CodexApprovals>) {
    assert!(matches!(
        &observed[..],
        [Observation::Changed((id, next))] if id == "local-thread" && *next == expected
    ));
}

#[test]
fn separates_automatic_manual_and_blocking_input_without_retaining_contents() {
    let mut observer = Observer::default();
    observer.streams.insert("local-thread".into(), None);
    assert_change(
        observer.frame(&snapshot("local-thread", 1, json!({
            "requests":[
                {"method":MANUAL,"params":{"command":"secret command"}},
                {"method":"item/tool/requestUserInput","params":{"isBlocking":true,"question":"secret question"}},
                {"method":"item/tool/requestUserInput","params":{"isBlocking":false}}
            ],
            "turns":[
                {"status":"completed","items":[{"type":"automaticApprovalReview","status":"inProgress"}]},
                {"status":"inProgress","items":[
                    {"type":"agentMessage","text":"secret answer"},
                    {"type":"automaticApprovalReview","status":"inProgress","reason":"secret reason"}
                ]}
            ]
        }))),
        Some(state(1, 1, 1)),
    );
    let stored = &observer.streams["local-thread"].as_ref().unwrap().state;
    assert!(!stored.to_string().contains("secret"));
    assert_change(
        observer.frame(&delta(
            "local-thread",
            1,
            json!([
                {"op":"remove","path":["requests",0]},
                {"op":"replace","path":["turns",1,"items",1,"status"],"value":"approved"},
                {"op":"replace","path":["turns",1,"items",0,"text"],"value":"secret delta"}
            ]),
        )),
        Some(state(0, 1, 0)),
    );
    assert_change(
        observer.frame(&delta(
            "local-thread",
            2,
            json!([
                {"op":"replace","path":["requests","length"],"value":0}
            ]),
        )),
        Some(state(0, 0, 0)),
    );
    assert!(!observer.streams["local-thread"]
        .as_ref()
        .unwrap()
        .state
        .to_string()
        .contains("secret"));
}

#[test]
fn requires_a_fresh_local_snapshot_after_a_gap_or_owner_loss() {
    let mut observer = Observer::default();
    observer.streams.insert("local-thread".into(), None);
    let empty = json!({"requests":[],"turns":[]});
    assert!(observer
        .frame(&snapshot("other-thread", 1, empty.clone()))
        .is_empty());
    let mut remote = snapshot("local-thread", 1, empty.clone());
    remote["params"]["hostId"] = json!("remote");
    assert!(observer.frame(&remote).is_empty());
    assert_change(
        observer.frame(&snapshot(
            "local-thread",
            3,
            json!({"requests":[{"method":MANUAL}],"turns":[]}),
        )),
        Some(state(1, 0, 0)),
    );
    assert!(observer
        .frame(&delta("local-thread", 2, json!([])))
        .is_empty());
    let output = observer.frame(&delta("local-thread", 4, json!([])));
    assert!(
        matches!(&output[..], [Observation::Changed((_, None)), Observation::Resync(id)] if id=="local-thread")
    );
    assert!(observer
        .frame(&delta("local-thread", 5, json!([])))
        .is_empty());
    assert_change(
        observer.frame(&snapshot("local-thread", 6, empty)),
        Some(state(0, 0, 0)),
    );
    assert_change(
        observer.frame(&json!({
            "type":"broadcast","method":"client-status-changed","sourceClientId":"desktop",
            "params":{"status":"disconnected"}
        })),
        None,
    );
}

#[test]
fn unknown_request_methods_count_as_manual_without_keeping_their_names() {
    let mut observer = Observer::default();
    observer.streams.insert("local-thread".into(), None);
    assert_change(
        observer.frame(&snapshot(
            "local-thread",
            1,
            json!({"requests":[{"method":"item/secretNewThing/requestApproval"}],"turns":[]}),
        )),
        Some(state(1, 0, 0)),
    );
    let stored = &observer.streams["local-thread"].as_ref().unwrap().state;
    assert!(!stored.to_string().contains("secretNewThing"));
}

#[test]
fn rejected_frames_are_counted_before_the_first_snapshot() {
    let mut observer = Observer::default();
    observer.streams.insert("local-thread".into(), None);
    let mut incompatible = snapshot("local-thread", 1, json!({"requests":[],"turns":[]}));
    incompatible["version"] = json!(SYNC_VERSION + 1);
    assert!(observer.frame(&incompatible).is_empty());
    assert!(observer
        .frame(&snapshot("local-thread", 2, json!({"turns":[]})))
        .is_empty());
    assert!(
        observer.rejected
            == Rejections {
                version: 1,
                shape: 1,
                revision: 0
            }
    );
}

async fn read_peer(server: &mut NamedPipeServer) -> Value {
    let len = server.read_u32_le().await.unwrap() as usize;
    let mut frame = vec![0; len];
    server.read_exact(&mut frame).await.unwrap();
    serde_json::from_slice(&frame).unwrap()
}

async fn write_peer(server: &mut NamedPipeServer, msg: Value) {
    let frame = serde_json::to_vec(&msg).unwrap();
    server
        .write_all(&(frame.len() as u32).to_le_bytes())
        .await
        .unwrap();
    server.write_all(&frame[..frame.len() / 2]).await.unwrap();
    server.write_all(&frame[frame.len() / 2..]).await.unwrap();
}

fn hook(event: &str) -> EnvelopeV2 {
    EnvelopeV2 {
        v: 2,
        id: uuid::Uuid::new_v4().to_string(),
        agent: Agent::Codex,
        event: event.into(),
        ts: 1,
        src: EnvelopeSrc {
            cli: "test".into(),
            pid: 1,
        },
        payload: if event == "PermissionRequest" {
            json!({"session_id":"local-thread","turn_id":"turn","tool_name":"Bash","tool_input":{"command":"fixture"}})
        } else {
            json!({"session_id":"local-thread","turn_id":"turn"})
        }
        .to_string(),
        parse_error: None,
    }
}

#[test]
fn named_pipe_delivery_drives_the_core_and_disconnect_clears_pending() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        timeout(Duration::from_secs(8), async {
            let path = format!(r"\\.\pipe\code-musume-test-{}", uuid::Uuid::new_v4());
            let mut server = ServerOptions::new().first_pipe_instance(true).create(&path).unwrap();
            let (_subscriptions, wanted) = watch::channel(vec!["local-thread".into()]);
            let (tx, mut updates) = mpsc::channel(8);
            let (rejections, _rejected) = watch::channel(Rejections::default());
            let (capture, capture_rx) = watch::channel(None);
            let client = tokio::spawn(async move {
                let _capture = capture;
                observe(&path, wanted, tx, rejections, capture_rx, None).await
            });
            server.connect().await.unwrap();
            let init = read_peer(&mut server).await;
            assert_eq!(init["method"], "initialize");
            assert_eq!(init["params"]["clientType"], "code-musume-observer");
            write_peer(&mut server, json!({
                "type":"response","requestId":init["requestId"],"resultType":"success",
                "result":{"clientId":"observer"}
            })).await;
            let follow = read_peer(&mut server).await;
            assert_eq!(follow["method"], "thread-stream-following-changed");
            assert_eq!(follow["params"], json!({"hostId":"local","conversationId":"local-thread","following":true}));

            let mut engine = Engine::new("test".into(), Config::default()).with_codex_approval_sync(true);
            engine.ingest(hook("UserPromptSubmit"), 0);
            engine.ingest(hook("PermissionRequest"), 1);
            // Before the desktop reports this session, the hook request is the only evidence.
            // The first snapshot below was taken after it and supersedes it.
            assert_eq!(engine.snapshot().state, DisplayState::WaitingPermission);
            let turn = "turn:private/session~key";
            let sequence = [
                (snapshot("local-thread", 1, json!({
                    "requests":[],"turns":[],
                    "turnHistory":{"history":{"entitiesByKey":{(turn):{"status":"inProgress","items":[]}}}}
                })), DisplayState::Thinking),
                (delta("local-thread", 1, json!([{
                    "op":"add","path":["turnHistory","history","entitiesByKey",turn,"items",0],
                    "value":{"type":"automaticApprovalReview","status":"inProgress","completedAtMs":null}
                }])), DisplayState::Working),
                (delta("local-thread", 2, json!([{
                    "op":"replace","path":["turnHistory","history","entitiesByKey",turn,"items",0],
                    "value":{"type":"automaticApprovalReview","status":"approved","completedAtMs":30}
                }])), DisplayState::Thinking),
                (delta("local-thread", 3, json!([{"op":"add","path":["requests",0],"value":{"method":MANUAL}}])), DisplayState::WaitingPermission),
                (delta("local-thread", 4, json!([{"op":"remove","path":["requests",0]}])), DisplayState::Thinking),
                (delta("local-thread", 5, json!([{"op":"add","path":["requests",0],"value":{"method":MANUAL}}])), DisplayState::WaitingPermission),
            ];
            for (step, (msg, expected)) in sequence.into_iter().enumerate() {
                write_peer(&mut server, msg).await;
                let (id, next) = updates.recv().await.unwrap();
                engine.update_codex_approvals(id, next, step as u64 + 2);
                assert_eq!(engine.snapshot().state, expected);
                assert_eq!(engine.snapshot().pending_count, u32::from(expected == DisplayState::WaitingPermission));
            }
            drop(server);
            let (id, next) = updates.recv().await.unwrap();
            assert!(next.is_none());
            engine.update_codex_approvals(id, next, 8);
            assert_eq!(engine.snapshot().state, DisplayState::Thinking);
            assert_eq!(engine.snapshot().pending_count, 0);
            client.abort();
        }).await.unwrap();
    });
}
