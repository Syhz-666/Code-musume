# 代码娘 · code-musume

Windows 桌面伴侣，接收 Claude Code 与本机 Codex 桌面应用的工作事件。同一角色可以随主导 agent 切换形态；轻量陪伴、默认静音、无遥测。

当前为 0.1.0 工程试用版。构建产物在 artifacts：Windows x64 安装包与便携 ZIP。Claude 正常流程已真实接入；Codex 桌面信任、真实双来源并发与完整日用仍待验收，详见 docs/验证记录-2026-10-14.md。

## 使用

启动代码娘，从托盘打开「设置与接入」。分别预览并写入 Claude / Codex 接线；在各自受信任项目中新开会话验证。Codex 定义需在 Codex 中审阅和信任，升级后重新检查；安装器不代替信任确认。

角色页可选择内置团子、设置自动形态、导入角色文件夹或 Codex PNG/WebP 图集。兼容 1536×1872(v1) / 1536×2288(v2)；图片 ≤20MiB，导入结构验证不等于对美术语义的认可。

托盘提供显示、隐藏、穿透、设置和退出。全屏自动让位，位置按物理坐标与显示器保存；减少动画尊重系统偏好。首版 Codex 验收仅本机桌面任务，CLI/IDE/远程后置。

## 开发

需要 Windows 11、WebView2、Rust MSVC（本机 1.99）、Node 和 pnpm（packageManager 锁定）。

```powershell
pnpm install
cargo build -p code-musume-hook
pnpm --filter @code-musume/pet tauri dev
```

先构建 CLI，保证它与 pet.exe 相邻。dev 使用 runtime.dev.json，release 使用 runtime.json，接线模式对应构建。

```powershell
pnpm contracts:generate
pnpm typecheck
pnpm test
pnpm lint
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm build
pnpm package:windows
```

package:windows 先构建 CLI，再构建 Tauri/NSIS；另输出双可执行文件便携 ZIP。安装器构建受 NSIS 工具与网络可用性影响，验证记录标注实际结果。

## 验证与接线 CLI

```powershell
target/debug/code-musume-hook.exe emit-fixture --agent codex --event PreToolUse --runtime dev
target/debug/code-musume-hook.exe install --agent codex --scope project --cwd D:/sandbox --runtime dev
```

install/uninstall 默认只预览；真正写入需要 --confirm 和此前 --expected-digest。支持两家独立接线。hook 模式安静 exit 0；显示子命令可输出人读结果。测试目录可通过 --state-dir 隔离回执。

tools/smoke.ps1 验证真实 CLI→HTTP→native 通路，输入为合成样本；tools/measure-runtime.ps1 采集进程树资源；tools/test-installer.ps1 在隔离目录验证当前用户安装卸载。真实脱敏 Claude 记录在 fixtures/hooks/captured-claude-2.1.295.json。Codex 信任、窗口细项和一天使用按验证清单人工完成。

## 结构与资料

- crates/contracts：共享 Rust 类型和 TS 生成。
- crates/core：唯一工作语义；纯函数与场景测试。
- hooks/cli：转发、截止、独立安装与回滚。
- apps/pet：Tauri 系统宿主与 TS 表现/设置。
- packages/protocol：生成的前端接口与修订过滤。

当前文档见 docs/README.md，接手入口 HANDOFF.md；2026-10-14 方案为现行，旧方案为历史。代码 MIT，导入素材保持原授权，原创正式素材另行确认。
