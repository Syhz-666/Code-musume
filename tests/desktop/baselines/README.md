# 截图基准

正常测试只生成候选截图，不自动接受新画面。

需要重新收集完整候选图时，使用 `pnpm test:ui -- --review-candidates`。此模式保留旧基准，以待审阅返回失败；不等于截图比较通过。

1. 确认 functional-results.json 全部通过、cold-start-results.json 两项通过。
2. 人工或经用户明确委托的代理查看完整截图；可使用 `python tools/ui-contact-sheet.py <运行目录>` 生成汇总图，原图保留细节。
3. 审阅完成后执行 `node tools/approve-ui-baselines.mjs --from <运行目录> --reviewer <审阅人> --reviewed`。
4. 重新执行 `pnpm test:ui`，比较新画面与已审阅基准。review.json 记录图片哈希和测试环境，基准变化须重新审阅。

初始图片的审阅状态以 review.json 为准。缺少图片或审阅记录属于待验收，测试返回失败。不能用自动截图替代审阅。

用户明确委托代理完成决策时，可附加 --delegated --authorization "授权原话"，记录真实审阅身份；这不代替 Codex 信任流程或系统界面的人工验收。

## 环境约束补充（2026-10-29）

截图使用 e2e 构建内的固定中文字体，说明与授权见 [fonts/README.md](../fonts/README.md)。
每个功能场景恢复配置与工作状态，截图差异先记录、在套件结束时统一失败，仍严格使用原有阈值。
重新生成候选不能直接算通过；本机审阅与比较通过之后，GitHub 托管 Windows 环境仍需单独验证。
