//! Real stdin/HTTP relay and Windows encoded-command verification, with isolated runtime files.
use code_musume_contracts::*;
use code_musume_hook::installer;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
struct Sandbox {
    path: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cm-relay-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let base = std::env::temp_dir().canonicalize().ok();
        let path = self.path.canonicalize().ok();
        if let (Some(base), Some(path)) = (base, path) {
            if path.starts_with(base)
                && self
                    .path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("cm-relay-"))
            {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }
}
fn server(path: &Path) -> std::thread::JoinHandle<EnvelopeV2> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    fs::write(
        path.join("runtime.dev.json"),
        serde_json::to_vec(&RuntimeFile {
            v: 2,
            port: listener.local_addr().unwrap().port(),
            token: "a".repeat(64),
            pid: std::process::id(),
            app_version: "test".into(),
            instance_id: "test".into(),
            started_at: 0,
        })
        .unwrap(),
    )
    .unwrap();
    std::thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(8);
        let mut conn = loop {
            match listener.accept() {
                Ok((c, _)) => break c,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < end => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => panic!("No relay connection: {e}"),
            }
        };
        conn.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut bytes = vec![];
        let mut buf = [0u8; 4096];
        let (header, len) = loop {
            let n = conn.read(&mut buf).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            if let Some(p) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                let h = std::str::from_utf8(&bytes[..p]).unwrap();
                assert!(h.to_ascii_lowercase().contains("authorization: bearer "));
                let len = h
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap();
                break (p + 4, len);
            }
        };
        while bytes.len() < header + len {
            let n = conn.read(&mut buf).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
        }
        conn.write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
        serde_json::from_slice(&bytes[header..header + len]).unwrap()
    })
}
fn run(mut cmd: Command, input: &str) -> std::process::Output {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let end = Instant::now() + Duration::from_secs(8);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > end {
            let _ = child.kill();
            panic!("Relay process hung");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}
#[test]
fn native_hook_transmits_exact_payload_and_silent_success() {
    let t = Sandbox::new();
    let handle = server(&t.path);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_code-musume-hook"));
    cmd.args([
        "--agent",
        "claude-code",
        "--event",
        "PreToolUse",
        "--runtime",
        "dev",
    ])
    .env("CODE_MUSUME_DATA_DIR", &t.path);
    let input = "{\n \"session_id\":\"s\",\"tool_input\":{\"command\":\"中文 路径\"}\n}";
    let out = run(cmd, input);
    assert!(out.status.success());
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    assert_eq!(handle.join().unwrap().payload, input);
}
#[cfg(windows)]
#[test]
fn codex_encoded_windows_command_forwards_utf8_and_special_path() {
    let t = Sandbox::new();
    let exe = t
        .path
        .join("中文 空格 & ' % !")
        .join("code-musume-hook.exe");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_code-musume-hook"), &exe).unwrap();
    let handle = server(&t.path);
    let handler = installer::handler(Agent::Codex, "PreToolUse", &exe, "dev");
    let command = handler["commandWindows"].as_str().unwrap();
    let encoded = command.split_whitespace().last().unwrap();
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-EncodedCommand", encoded])
        .env("CODE_MUSUME_DATA_DIR", &t.path);
    let input =
        "{\"session_id\":\"s\",\"turn_id\":\"t\",\"tool_input\":{\"command\":\"中文 & % ! 路径\"}}";
    let out = run(cmd, input);
    assert!(out.status.success(), "{:?}", out.status);
    assert!(
        out.stdout.is_empty() && out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let envelope = handle.join().unwrap();
    assert_eq!(envelope.agent, Agent::Codex);
    assert_eq!(envelope.payload, input);
}
#[test]
fn stalled_stdin_still_exits_silently_within_deadline() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_code-musume-hook"))
        .args(["--agent", "codex", "--event", "Stop", "--budget-ms", "100"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _held = child.stdin.take();
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "deadline watchdog failed"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert!(start.elapsed() < Duration::from_millis(700));
}
