//! code-musume-hook —— 哑管道 CLI（Phase 1 骨架）。
//!
//! 红线（架构 P3）：hook 模式**任何路径 exit 0**、**不写 stdout（hook 模式）**、静默失败、不做决策。
//! 完整实现（信封组装 / HTTP POST / install / emit-fixture / --dump）在 Phase 2。

fn main() {
    // Phase 1 仅保证：可编译、可被调用、永远安静地 exit 0。
    std::process::exit(0);
}
