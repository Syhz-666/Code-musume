use code_musume_contracts::*;
use code_musume_hook::{installer, runtime};
use std::{
    io::{self, Read},
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
fn option(a: &[String], k: &str) -> Option<String> {
    a.windows(2).find(|p| p[0] == k).map(|p| p[1].clone())
}
fn send(e: &EnvelopeV2, path: PathBuf, budget: Duration) -> Result<(), String> {
    let r = runtime::read_runtime(&path)?;
    let bytes = serde_json::to_vec(e).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err("信封过大".into());
    }
    let client: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(budget))
        .max_redirects(0)
        .proxy(None)
        .build()
        .into();
    client
        .post(format!("http://127.0.0.1:{}/v2/event", r.port))
        .header("Authorization", format!("Bearer {}", r.token))
        .header("Content-Type", "application/json")
        .header("Connection", "close")
        .send(bytes.as_slice())
        .map_err(|_| "桌宠接收失败".to_string())?;
    Ok(())
}
fn hook(a: &[String]) {
    std::panic::set_hook(Box::new(|_| {}));
    let budget = option(a, "--budget-ms")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(300)
        .clamp(1, 300);
    if std::thread::Builder::new()
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(budget));
            std::process::exit(0);
        })
        .is_err()
    {
        std::process::exit(0);
    }
    let _ = std::panic::catch_unwind(|| {
        let start = Instant::now();
        let agent = Agent::parse(&option(a, "--agent").unwrap_or_default())?;
        let event = option(a, "--event")?;
        let mut bytes = Vec::new();
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        let parse_error = if bytes.len() > MAX_INPUT_BYTES {
            bytes.truncate(MAX_INPUT_BYTES);
            Some(ParseError {
                kind: "size".into(),
            })
        } else if std::str::from_utf8(&bytes)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .is_none()
        {
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
            id: uuid::Uuid::new_v4().to_string(),
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
            let _ = send(&e, path, remaining);
        }
        Some(())
    });
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
