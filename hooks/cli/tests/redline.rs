//! hook CLI 红线集成测试：**任何路径 exit 0** + **hook 模式不写 stdout**。
//!
//! 机制：Cargo 为集成测试设置 `CARGO_BIN_EXE_<bin 目标名>`（连字符原样保留），
//! 并在存在集成测试时自动构建 bin（Cargo Book · environment-variables / cargo-targets）。
//! 红线出处：`docs\架构设计-2026-10-11.md` P3、`HANDOFF.md` §4。
//!
//! 运行方式（注意 -p）：`cargo test -p code-musume-hook`
//! —— 根目录裸跑 `cargo test` 因 default-members 不覆盖本包。
//!
//! 注意：标识符保持 ASCII —— clippy 的 `mixed_script_confusables` 会在 `-D warnings` 下拦截。

use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// bin 目标名 = code-musume-hook（见 hooks/cli/Cargo.toml 的 [[bin]]）。
const BIN: &str = env!("CARGO_BIN_EXE_code-musume-hook");

/// 看门狗：红线最危险的失败模式不是 exit!=0，而是「挂住」——hook 阻塞 Claude Code。
const WATCHDOG: Duration = Duration::from_secs(5);

/// 跑一次 CLI：喂 stdin（`None` = 立刻关闭 = EOF），等退出。
/// 超时 → kill + panic（把「挂住」变成测试失败而不是 CI 卡死）。
fn run_hook(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // 与开发者 shell 解耦：CODE_MUSUME_DEBUG 会打开 CLI 日志（架构 §9.3）
        .env_remove("CODE_MUSUME_DEBUG")
        // 与 CWD 解耦（Phase 2 的 runtime 文件发现可能读 CWD 相关路径）
        .current_dir(env!("CARGO_MANIFEST_DIR"));

    let mut child = cmd.spawn().expect("spawn code-musume-hook");

    match stdin {
        Some(bytes) => {
            let mut si = child.stdin.take().expect("stdin piped");
            // 子进程可能先退出（不读 stdin）→ 写入报 EPIPE 是允许的，忽略
            let _ = si.write_all(bytes);
            let _ = si.flush();
            drop(si); // 关闭管道 = EOF
        }
        None => drop(child.stdin.take()),
    }

    let deadline = Instant::now() + WATCHDOG;
    loop {
        match child.try_wait().expect("try_wait") {
            Some(_) => break,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let out = child.wait_with_output().expect("wait_with_output");
                panic!(
                    "hook CLI 未在 {WATCHDOG:?} 内退出（红线：必须毫秒级 exit 0）args={args:?} \
                     stdout={:?} stderr={:?}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    child.wait_with_output().expect("wait_with_output")
}

/// 红线断言：exit 0 + stdout 空 + stderr 空（默认静默）。
fn assert_silent_zero(args: &[&str], stdin: Option<&[u8]>, label: &str) {
    let out = run_hook(args, stdin);
    assert_eq!(
        out.status.code(),
        Some(0),
        "[{label}] 必须 exit 0（P3 红线），实际 {:?}",
        out.status
    );
    assert!(
        out.stdout.is_empty(),
        "[{label}] hook 模式 stdout 必须为空（stdout 会进模型上下文），实际: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        out.stderr.is_empty(),
        "[{label}] 默认（无 CODE_MUSUME_DEBUG）stderr 必须为空，实际: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn hook_mode_canonical_invocation_is_silent_and_ok() {
    assert_silent_zero(
        &["--agent", "claude-code", "--event", "PreToolUse"],
        Some(br#"{"hook_event_name":"PreToolUse","session_id":"s1","tool_name":"Read"}"#),
        "规范调用",
    );
}

#[test]
fn hook_mode_malformed_stdin_is_silent_and_ok() {
    // 非 JSON
    assert_silent_zero(
        &["--agent", "claude-code", "--event", "Stop"],
        Some(b"not-json"),
        "非 JSON",
    );
    // 截断 JSON
    assert_silent_zero(
        &["--agent", "claude-code", "--event", "Stop"],
        Some(b"{\"a\":"),
        "截断 JSON",
    );
    // 空 stdin / EOF
    assert_silent_zero(
        &["--agent", "claude-code", "--event", "Stop"],
        None,
        "空 stdin",
    );
    // 非 UTF-8 字节（Phase 2 的 String::from_utf8 若直接 unwrap 会 panic —— 提前防线）
    assert_silent_zero(
        &["--agent", "claude-code", "--event", "Stop"],
        Some(&[0xFF, 0xFE, 0x00, 0x80]),
        "非 UTF-8",
    );
}

#[test]
fn hook_mode_no_args_is_silent_and_ok() {
    assert_silent_zero(&[], None, "无参数");
}

#[test]
fn hook_mode_unknown_args_is_silent_and_ok() {
    assert_silent_zero(&["--bogus", "--event"], Some(b"x"), "未知参数");
    assert_silent_zero(&["--agent", "claude-code"], Some(b"x"), "缺 --event");
}

#[test]
fn hook_mode_garbage_bytes_never_panic() {
    // 二进制垃圾（含 NUL）
    let junk: Vec<u8> = (0u8..=255).cycle().take(512).collect();
    assert_silent_zero(
        &["--agent", "claude-code", "--event", "SessionEnd"],
        Some(&junk),
        "二进制垃圾",
    );
}

#[test]
fn hook_mode_large_stdin_is_silent_and_ok() {
    // 32KB（< 协议 64KB 上限）：保证不会因管道写阻塞而变慢/挂住
    let big = vec![b'a'; 32 * 1024];
    assert_silent_zero(
        &["--agent", "claude-code", "--event", "PostToolUse"],
        Some(&big),
        "32KB stdin",
    );
}

#[test]
fn hook_mode_paths_with_spaces_and_cjk_are_ok() {
    // P19 的轻量前哨：Windows 下 argv 含空格+中文（真实安装路径形态）
    assert_silent_zero(
        &[
            "--agent",
            "claude-code",
            "--event",
            "PreToolUse",
            "--cwd",
            r"C:\Users\admin\我的 项目",
        ],
        Some(br#"{"session_id":"s1"}"#),
        "中文+空格路径参数",
    );
}
