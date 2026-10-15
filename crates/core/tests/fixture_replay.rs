//! Expectations are reviewed against the state specification, never regenerated from Engine.
use code_musume_contracts::*;
use code_musume_core::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scenario {
    schema_version: u32,
    id: String,
    description: String,
    provenance: String,
    source_versions: BTreeMap<String, String>,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Step {
    at_ms: u64,
    action: Action,
    expect: Value,
    diagnostic: Option<String>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
enum Action {
    Event {
        agent: Agent,
        event: String,
        payload: Value,
        id: String,
        #[serde(default)]
        ts: u64,
    },
    Tick,
    Lost {
        count: u64,
    },
    Configure {
        config: Config,
    },
    Reset,
}
fn compare(expected: &Value, actual: &Value, path: &str, errors: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (key, value) in e {
                match a.get(key) {
                    Some(actual) => compare(value, actual, &format!("{path}.{key}"), errors),
                    None => errors.push(format!("{path}.{key}: missing; expected {value}")),
                }
            }
        }
        (Value::Array(e), Value::Array(a)) if e.len() == a.len() => {
            for (i, (e, a)) in e.iter().zip(a).enumerate() {
                compare(e, a, &format!("{path}[{i}]"), errors);
            }
        }
        _ if expected == actual => {}
        _ => errors.push(format!("{path}: expected {expected}; actual {actual}")),
    }
}
#[test]
fn replay_reviewed_fixtures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios");
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no replay fixtures found");
    let mut reports = vec![];
    let mut failed = 0;
    for path in paths {
        let scenario: Scenario = serde_json::from_slice(&fs::read(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(scenario.schema_version, 1);
        assert!(matches!(
            scenario.provenance.as_str(),
            "synthetic" | "real-redacted" | "derived-from-capture"
        ));
        assert!(!scenario.description.is_empty() && !scenario.source_versions.is_empty());
        assert!(!scenario.steps.is_empty());
        let mut engine = Engine::new("fixture-replay".into(), Config::default());
        let mut previous = 0;
        let mut checkpoints = vec![];
        for (index, step) in scenario.steps.iter().enumerate() {
            assert!(
                step.at_ms >= previous,
                "{}: time moved backwards",
                scenario.id
            );
            previous = step.at_ms;
            for key in [
                "state",
                "certainty",
                "dominantAgent",
                "pendingCount",
                "formId",
            ] {
                assert!(
                    step.expect.get(key).is_some(),
                    "{} step {}: missing expectation {key}",
                    scenario.id,
                    index + 1
                );
            }
            match &step.action {
                Action::Event {
                    agent,
                    event,
                    payload,
                    id,
                    ts,
                } => {
                    let other_turn = if *agent == Agent::ClaudeCode {
                        "turn_id"
                    } else {
                        "prompt_id"
                    };
                    assert!(
                        payload.get(other_turn).is_none(),
                        "{}: payload mixes agent turn fields",
                        scenario.id
                    );
                    assert!(scenario.source_versions.contains_key(agent.as_str()));
                    engine.ingest(
                        EnvelopeV2 {
                            v: 2,
                            id: id.clone(),
                            agent: *agent,
                            event: event.clone(),
                            ts: *ts,
                            src: EnvelopeSrc {
                                cli: "fixture".into(),
                                pid: 0,
                            },
                            payload: if let Value::String(raw) = payload {
                                raw.clone()
                            } else {
                                payload.to_string()
                            },
                            parse_error: None,
                        },
                        step.at_ms,
                    );
                }
                Action::Tick => {
                    engine.tick(step.at_ms);
                }
                Action::Lost { count } => engine.lost(*count, step.at_ms),
                Action::Configure { config } => {
                    engine.config = config.clone();
                    engine.tick(step.at_ms);
                }
                Action::Reset => engine.reset(step.at_ms),
            }
            let actual = serde_json::to_value(engine.snapshot()).unwrap();
            let mut errors = vec![];
            compare(&step.expect, &actual, "projection", &mut errors);
            if let Some(wanted) = &step.diagnostic {
                let got = engine.diagnostics().last().map(|d| d.outcome.clone());
                if got.as_ref() != Some(wanted) {
                    errors.push(format!("diagnostic: expected {wanted}; actual {got:?}"));
                }
            }
            if !errors.is_empty() {
                failed += 1;
                eprintln!(
                    "FAIL {} / step {} / {}ms\n  {}",
                    scenario.id,
                    index + 1,
                    step.at_ms,
                    errors.join("\n  ")
                );
            }
            checkpoints.push(json!({"step":index+1,"atMs":step.at_ms,"passed":errors.is_empty(),"errors":errors,"expected":step.expect,"actual":actual}));
        }
        println!("{}: {} checkpoints", scenario.id, scenario.steps.len());
        reports.push(
            json!({"id":scenario.id,"provenance":scenario.provenance,"checkpoints":checkpoints}),
        );
    }
    let out = std::env::var_os("CODE_MUSUME_REPLAY_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../.test-artifacts/replay/result.json")
        });
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    fs::write(
        &out,
        serde_json::to_vec_pretty(
            &json!({"schemaVersion":1,"failedCheckpoints":failed,"scenarios":reports}),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(failed, 0, "replay failed; report: {}", out.display());
}
