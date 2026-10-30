use code_musume_contracts::*;
use code_musume_hook::diagnostics::HookReport;
use code_musume_hook::{installer, runtime};
use std::{
    io::{self, Read},
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
fn option(a: &[String], k: &str) -> Option<String> {
    a.windows(2).find(|p| p[0] == k).map(|p| p[1].clone())
}
struct RelayFailure {
    stage: &'static str,
    code: &'static str,
    status: Option<u16>,
}
fn relay(e: &EnvelopeV2, path: PathBuf, budget: Duration) -> Result<u16, RelayFailure> {
    let r = runtime::read_runtime(&path).map_err(|_| RelayFailure {
        stage: "runtime",
        code: "runtime-unavailable",
        status: None,
    })?;
    let bytes = serde_json::to_vec(e).map_err(|_| RelayFailure {
        stage: "serialize",
        code: "serialization-failed",
        status: None,
    })?;
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(RelayFailure {
            stage: "serialize",
            code: "envelope-too-large",
            status: None,
        });
    }
    let client: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(budget))
        .max_redirects(0)
        .proxy(None)
        .build()
        .into();
    let response = client
        .post(format!("http://127.0.0.1:{}/v2/event", r.port))
        .header("Authorization", format!("Bearer {}", r.token))
        .header("Content-Type", "application/json")
        .header("Connection", "close")
        .send(bytes.as_slice())
        .map_err(|error| {
            let (code, status) = match error {
                ureq::Error::StatusCode(status) => ("http-rejected", Some(status)),
                ureq::Error::Timeout(_) => ("http-timeout", None),
                ureq::Error::Io(_) | ureq::Error::ConnectionFailed => ("connection-failed", None),
                _ => ("http-error", None),
            };
            RelayFailure {
                stage: "http",
                code,
                status,
            }
        })?;
    Ok(response.status().as_u16())
}
fn send(e: &EnvelopeV2, path: PathBuf, budget: Duration) -> Result<(), String> {
    relay(e, path, budget)
        .map(|_| ())
        .map_err(|e| format!("桌宠接收失败: {}", e.code))
}
fn hook(a: &[String]) {
    std::panic::set_hook(Box::new(|_| {}));
    let budget = option(a, "--budget-ms")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(300)
        .clamp(1, 300);
    let start = Instant::now();
    if std::thread::Builder::new()
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(budget));
            std::process::exit(0);
        })
        .is_err()
    {
        std::process::exit(0);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let agent = Agent::parse(&option(a, "--agent").unwrap_or_default());
    let event = option(a, "--event");
    let mut report = HookReport::new(&id, agent, event.as_deref().unwrap_or(""), budget);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let agent = agent?;
        let event = event?;
        report.stage("stdin-read", start);
        let mut bytes = Vec::new();
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        report.input = Some(diagnostics::InputDiagnostics::inspect(&bytes));
        report.stage("input-inspected", start);
        let parse_error = if bytes.len() > MAX_INPUT_BYTES {
            bytes.truncate(MAX_INPUT_BYTES);
            Some(ParseError {
                kind: "size".into(),
            })
        } else if !report.input.as_ref()?.json_valid {
            Some(ParseError {
                kind: "json".into(),
            })
        } else {
            None
        };
        let mut payload = String::from_utf8_lossy(&bytes).into_owned();
        while payload.len() > MAX_INPUT_BYTES {
            payload.pop();
        }
        let e = EnvelopeV2 {
            v: 2,
            id,
            agent,
            event,
            ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_millis() as u64,
            src: EnvelopeSrc {
                cli: env!("CARGO_PKG_VERSION").into(),
                pid: std::process::id(),
            },
            payload,
            parse_error,
        };
        let path = option(a, "--runtime-file")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                runtime::runtime_path(&option(a, "--runtime").unwrap_or("release".into()))
            });
        let remaining = Duration::from_millis(budget).saturating_sub(start.elapsed());
        if !remaining.is_zero() {
            report.stage("relay-start", start);
            match relay(&e, path, remaining) {
                Ok(status) => {
                    report.outcome = "http-delivered";
                    report.http_status = Some(status);
                    report.stage("http-complete", start);
                }
                Err(error) => {
                    report.outcome = error.code;
                    report.http_status = error.status;
                    report.stage(error.stage, start);
                }
            }
        } else {
            report.outcome = "budget-exhausted-before-send";
        }
        Some(())
    }));
    if result.is_err() {
        report.outcome = "panic-caught";
    }
    if let Some(path) = option(a, "--diagnostic-file") {
        report.save(&PathBuf::from(path), start);
    }
    std::process::exit(0);
}
fn administrative(a: &[String]) -> Result<(), String> {
    let command = &a[0];
    let agent =
        Agent::parse(&option(a, "--agent").unwrap_or("claude-code".into())).ok_or("无效 agent")?;
    let mode = option(a, "--runtime").unwrap_or("release".into());
    if !matches!(mode.as_str(), "dev" | "release") {
        return Err("runtime 必须为 dev 或 release".into());
    }
    if command == "cleanup-installs" {
        let count = installer::cleanup_filtered(
            &option(a, "--state-dir")
                .map(PathBuf::from)
                .unwrap_or_else(runtime::data_dir),
            &mode,
            &std::env::current_exe().map_err(|e| e.to_string())?,
            option(a, "--agent").map(|_| agent),
        )?;
        println!("已清理 {count} 处接入");
        return Ok(());
    }
    if command == "emit-fixture" {
        let event = option(a, "--event").unwrap_or("PreToolUse".into());
        let payload = if let Some(file) = option(a, "--file") {
            std::fs::read_to_string(file).map_err(|e| e.to_string())?
        } else {
            serde_json::json!({"session_id":option(a,"--session").unwrap_or("demo".into()),"prompt_id":"demo-turn","turn_id":"demo-turn","tool_use_id":"demo-tool","tool_name":"Bash","tool_input":{"command":"demo"}}).to_string()
        };
        let e = EnvelopeV2 {
            v: 2,
            id: uuid::Uuid::new_v4().to_string(),
            agent,
            event,
            ts: 0,
            src: EnvelopeSrc {
                cli: env!("CARGO_PKG_VERSION").into(),
                pid: std::process::id(),
            },
            payload,
            parse_error: None,
        };
        send(&e, runtime::runtime_path(&mode), Duration::from_millis(300))?;
        println!("事件已发送");
        return Ok(());
    }
    let root = if option(a, "--scope").as_deref() == Some("user") {
        PathBuf::from(std::env::var_os("USERPROFILE").ok_or("找不到用户目录")?)
    } else {
        option(a, "--cwd")
            .map(PathBuf::from)
            .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?)
    };
    let o = installer::Options {
        agent,
        root,
        exe: std::env::current_exe().map_err(|e| e.to_string())?,
        runtime: mode,
        state_dir: option(a, "--state-dir")
            .map(PathBuf::from)
            .unwrap_or_else(runtime::data_dir),
    };
    if command == "status" {
        println!(
            "{}",
            serde_json::to_string_pretty(&installer::status(&o)?).unwrap()
        );
    } else {
        let p = installer::preview(&o, command)?;
        if a.iter().any(|s| s == "--confirm") {
            let d = option(a, "--expected-digest").ok_or("写入需要此前预览的 --expected-digest")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&installer::apply(&o, command, &d)?).unwrap()
            );
        } else {
            println!("{}", serde_json::to_string_pretty(&p).unwrap());
        }
    }
    Ok(())
}
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.first().is_some_and(|s| {
        matches!(
            s.as_str(),
            "install" | "uninstall" | "status" | "emit-fixture" | "cleanup-installs"
        )
    }) {
        if let Err(e) = administrative(&a) {
            eprintln!("{e}");
            std::process::exit(1);
        }
    } else {
        hook(&a);
    }
}
