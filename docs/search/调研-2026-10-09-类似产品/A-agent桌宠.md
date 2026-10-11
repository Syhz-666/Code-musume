# A 路调研：AI 编码 agent 的桌宠 / 桌面伴侣（竞品与类似产品）

- **调研时间**：2026-10-09（所有 star / 提交时间均以此日为准）
- **调研范围**：把「桌宠 / 桌面伴侣」与「AI 编码 agent 状态感知」结合的项目与产品（GitHub、npm、Steam、官网）
- **调研方式**：以 GitHub REST API（`api.github.com/repos/...`）**逐仓库实测** star、created_at、pushed_at、license、description；用 API 读取 README 原文（base64 解码）核对功能原话。非 GitHub 来源（Steam、官网、npm、博客）只作线索，标注为二手/未核实。
- **环境限制（重要）**：本次会话 WebFetch 对所有域名不可用；`raw.githubusercontent.com` 与 `github.com` 网页在 curl 下返回 000（不可达）；只有 `api.github.com` 可直连。GitHub 匿名 API 配额 60 次/小时，调研后段触发 403 限流，个别条目（见「未能核实」）因此未拿到一手数据。
- **基线说明**：本报告核实并更新了 2026-09 那份基线；基线中记录的部分项目名/数字与现状有出入，下文逐条标注。

---

## 0. 结论速览（给产品决策用的三句话）

1. **赛道已经非常拥挤，且 2026-05 之后是爆发期**。仅本次能一手核实的、做「感知 AI 编码 agent 状态的桌宠」的项目就有 **40+ 个**，其中 star 过千的有 6 个、过 500 的有 9 个。2026 年 3–10 月，几乎每个月都有新项目进场。
2. **「多 agent 工具支持」早已不是差异点**：clawd-on-desk 一家就列了 27 种 agent 工具；vibe-pet、agentpet、oc-claw、UniPet、CoPet、all-pet 等都支持 5–15 种。
3. **⚠️ 最关键的发现——我们设想的「独特卖点」已被至少 4 个项目部分实现**：
   - **OC-Claw**（README 原话）：「Custom character animations, **pair different agents with different characters**」
   - **OpenPets**（README 原话）：可从内置 Pet Gallery「**configure which pet monitors each workspace or agent window**」
   - **HermesPet**（README 原话）：「**桌宠会跟随当前 AI 模式切换**」（7 类 AI 终端）
   - **Vibe Pet**（README 原话）：「every active editor or AI coding agent gets **its own pet card**」

   但有一个**仍然空着的位置**：现有实现全部是「**不同 agent → 换一只不同的角色/宠物**」（配置式、多角色），**没有发现任何一个做「同一个原创角色 → 在不同 agent 下呈现不同面貌/姿态/皮肤」**（同一角色的多形态映射）。差别虽细，但这是我们原始定义里「差异化形象」的字面含义——见第 5 节。

---

## 1. 核实后的基线项目现状（2026-10-09 一手数据）

| 名称 | 链接 | 状态 / star（2026-10-09） | 最近提交 | 许可证 | 现状要点 |
|---|---|---|---|---|---|
| **clawd-on-desk** | github.com/rullerzhou-afk/clawd-on-desk · clawdondesk.com | **6,400★** / 678 fork | **2026-10-09（当天）** | AGPL-3.0 | 基线记 6.2k★，现 6.4k★。**仍是赛道头部且极度活跃**。README 明确列出 **27 种 agent 工具**（Claude Code、Codex CLI、Copilot CLI、Gemini CLI、Antigravity、Cursor Agent、CodeBuddy、WorkBuddy、Grok Build、Kiro CLI、Kimi Code CLI、Qwen Code、ZCode、CodeWhale、opencode、MiMo Code、Pi、OMP、OpenClaw、Hermes Agent、Qoder、QoderWork、QwenWork、Reasonix CLI、DeepSeek Harness、TraeCode、MiniMax Code）。三种内置主题 Clawd（像素蟹）/ Calico（三花猫）/ Cloudling（云宝），另有官方可选主题 Hash Sage、Whale-chan，可自定义主题，可**导入 Codex Pet 动画包**。权限气泡、多会话 Dashboard、mini 模式。 |
| **clawd-conduit** | github.com/ssssssanjiu/clawd-conduit | **180★** / 10 fork | 2026-09-09（等于创建日） | MIT | 只是 clawd-on-desk 的接线层：把 Claude Code **全部 15 个 hook 事件**接上（14 个 command hook + 1 个阻塞式 HTTP `PermissionRequest`）。**创建当天推完就没再更新**，属一次性工具。基线里「把 Clawd 接进 15 个 hook」属实。 |
| **agentpet** | github.com/ntd4996/agentpet | **375★** / 65 fork | **2026-10-09** | MIT | 活跃。Swift/SwiftUI，macOS 13+ / Windows / Linux。菜单栏监控多 agent + 桌面像素宠物。**11 个 coding agent**。特色是**养成**（真实 token 喂食、升级、14 个成就、排行榜）+ **per-project pets（按项目分宠物）**。 |
| **oc-claw** | github.com/rainnoon/oc-claw · oc-claw.ai | **355★** | 2026-06-17 | 未取得（API 限流） | 创建 2026-03-19。**约 4 个月未更新**。Tauri v2 + Rust。支持 OpenClaw / Claude Code / Codex / Cursor / Gemini CLI / Hermes Agent（+ 远程 SSH）。Slogan 本身就写着「OC = OpenClaw. **OC = Original Character. Make it yours.**」⭐ **README 明确写明「pair different agents with different characters」**。Product Hunt 上线过。 |
| **OpenPets** | github.com/OpenPetsHQ/openpets | **1,274★** / 119 fork | **2026-10-09** | MIT | 定位是**平台**而非单个宠物：「Local first, desktop companion platform with animated pets, **plugin SDK v3** and coding-agent integrations」（Boring Dystopia Development 出品）。宠物层与 agent 层解耦：Claude Code / OpenCode / Cursor / Pi / 任意 MCP client 都能驱动它。⭐ **可从内置 Pet Gallery「configure which pet monitors each workspace or agent window」**。 |
| **Agentic-Desktop-Pet** | github.com/jihe520/Agentic-Desktop-Pet | **338★** / 45 fork | **2026-01-10** | 无 license | **已停更约 9 个月**。定位从「Agentic 桌宠」转为 LLM+记忆+情感+RPG 的独立桌宠项目，与 coding agent 状态感知的关系已弱化。 |
| **aemeath_withclaude** | github.com/77wliNd/aemeath_withclaude | **192★** / 13 fork | 2026-05-26 | MIT | 停滞约 4.5 个月。像素「爱弥斯」（**鸣潮 Wuthering Waves 的现成 IP 角色**，非原创）。HTTP hooks + MCP 双向：Claude 可通过 MCP 主动向用户提问（文本/确认/下拉），右键可给 Claude 发消息。端口 9527。 |
| **Hopet** | github.com/BinaryFroggy/Hopet | **63★** / 4 fork | 2026-07-27 | MIT | 停滞约 2.5 个月。macOS 14+（Swift）。**只支持 Claude Code + Codex CLI**（含各自 VS Code 扩展）。8 态状态机、多 session 聚合、权限与 AskUserQuestion 可在桌宠气泡 / 灵动岛内直接作答、Unix Domain Socket IPC。默认内置主题 **Hopi（像素小海豹）**，可导入 Codex pet 文件夹/ZIP 换形象。 |
| **dsh-pet**（基线只记了 dsh-pet-indesktop） | github.com/PC2005-cloud/dsh-pet | **1,184★** / 87 fork | **2026-10-09** | MIT | **DeepSeek Harness 官方插件生态里的桌宠**。npm 包 `dsh-pet`。100+ 手绘风透明动画、随机动作、屏幕漫游、多开、可加**全新宠物种类（pet pack）**。还配套「提示词配方 → 素材生成链 → 插件」三件套，可以让用户用 AI 视频**从零生成自己的宠物**。 |
| **dsh-pet-indesktop** | github.com/MerZlin/dsh-pet-indesktop | **822★** / 57 fork | 2026-10-06 | MIT | 基于 PC2005-cloud/dsh-pet 移植到 Windows/Linux/macOS 的版本（蓝色大肥鱼），并额外实现交互功能。 |
| **claw-universe/desktop-pet-notify** | — | **未能核实** | — | — | 用仓库名直查超时/限流，关键词搜索（claw-universe / desktop-pet-notify）只找到同名无关或 0★ 空仓库。**基线这条记录本次无法证实，疑为记录有误或已删库**。 |

**基线核实的净结论**：基线里 8 条中 7 条仍能对上（clawd-on-desk、clawd-conduit、agentpet、oc-claw、openpets、Agentic-Desktop-Pet、aemeath、dsh-pet-indesktop、Hopet），**但两个数字/描述要更新**：① clawd-on-desk 已从 6.2k 涨到 6.4k，且支持的工具从「10+」变成明确 27 种；② 基线漏掉了两个更大的玩家——**cc-haha（14.9k★）与 petdex（4.2k★）**（见下节）；③ claw-universe/desktop-pet-notify 查无此物。

---

## 2. 新入局者 / 之前未纳入的玩家（2025–2026）

### 2.1 第一梯队：规模已远超多数基线项目

| 名称 | 链接 | 状态 / star | agent 集成 | 形象机制 | 备注 |
|---|---|---|---|---|---|
| **cc-haha** | github.com/NanmiCoder/cc-haha · cchaha.ai | **14,932★** / 8,603 fork，创建 2026-03-31，**2026-10-09 仍活跃**，MIT | Claude Code 为核心（桌面工作台），含多模型（Claude/ChatGPT/Grok/本地端点）、Agent Teams、Computer Use | 内置 **4 个自研角色：搭搭、弧弧、补补、回回**，随任务状态换动作；可自制/导入（透明 PNG/WebP 单图，或 1536×2288 v2 逐帧图集）；换角色时已打开窗口立即同步 | **这是本次调研发现的最大玩家**：一个完整桌面工作台（多会话、worktree、Diff 审阅、技能市场、IM 接入…），桌宠只是其中一个特性（**默认关闭**）。生态位是「IDE/工作台」，不是「纯桌宠」，但它的桌宠已经是自研原创角色 + 可导入 |
| **petdex** | github.com/crafter-station/petdex · petdex.dev | **4,215★** / 209 fork，创建 2026-05-02，2026-09-28 更新，MIT | 桌面 app 里「connect your coding agents under **Agents**」；面向 Codex、Claude Code、DeepSeek Harness、Hermes、OpenCode、Gemini CLI 等 | **宠物画廊（Pokédex 式）+ CLI 一键安装 + 桌面 app 三件套**。宠物 = 一个文件夹（`pet.json` + `spritesheet.webp`，8×9 或 v2 8×11 帧）。选宠物在 Settings → **Pets**；社区已提交数千只 | ⭐ **它做的是「宠物分发基础设施」，而不是某个角色**。21 个开源项目已构建在它的 HTTP API / 包格式之上。对我们意味着：**「原创角色」本身很难成为壁垒，因为宠物市场已经商品化** |
| **super-agent-party** | github.com/heshengtao/super-agent-party | **2,723★**，创建 2025-03-08，2026-08-23 更新，AGPL-3.0 | 自托管 AI 伴侣，适配 Claude Code / Qwen Code 等工具链；有 Live2D / VRM（VMC、WebXR）扩展 | Live2D / VRM 虚拟形象（可换模型），Docker 全平台 | 定位偏「neuro-sama 式 AI 伴侣」，coding agent 只是它可接的一类工具，与我们的定位交叉但重心不同 |

### 2.2 第二梯队：与本次产品定位高度重叠

| 名称 | 链接 | 状态 / star | agent 集成 | 形象机制 | 备注 |
|---|---|---|---|---|---|
| **HermesPet** | github.com/basionwang-bot/HermesPet · hermespet.cc | **598★**，创建 2026-05-15，2026-09-23 更新，DMG 已签名公证 | **7 类 AI 终端**：Cloud 在线模型 / Hermes / OpenClaw / **Claude Code / Codex / QwenCode / Kimi**（后四者复用本机登录态） | ⭐ **「桌宠会跟随当前 AI 模式切换」**（README 原话）。二手来源称内置 5 只像素桌宠（Clawd、云朵、fomo、Pegasus、coco）——**该列表未在 README 一手核实** | macOS 14+ 原生（Swift 6/SwiftUI），常驻刘海 + 屏幕边缘控制台 + 系统遥测；手机端/手表端在开发。实际是「刘海 AI 客户端 + 桌宠」 |
| **MiniCPM-Desk-Pet**（OpenBMB 官方） | github.com/OpenBMB/MiniCPM-Desk-Pet | **492★**，创建 2026-05-18，2026-08-18 更新，AGPL-3.0 | 自动扫描本机编程 agent：**Cursor、Claude Code、Codex**（以及 Gemini、Copilot、Kiro、CodeBuddy 等），一键连接 | 本地 MiniCPM5-1B 驱动；**LoRA 人格适配器可切换/导入**（PEFT → GGUF → App 上传）；可换 PNG/APNG 动画与音效 | **UI 基于 clawd-on-desk 的 Electron 分支二次开发**——说明本项目已被当作底座的底座。卖点是「本地小模型 + 桌宠人格」，不是形象差异化 |
| **buddy**（fiorastudio） | github.com/fiorastudio/buddy | **115★**，创建 2026-04-09，**2026-10-08 活跃**，MIT | Claude Code、Codex、Cursor CLI、GitHub Copilot 等 | 虚拟宠物，带性格的**代码评审反馈**，可升级养成 | 偏「AI 同事/评审宠物」，赛道相邻 |
| **LLMPET** | github.com/myunwang/LLMPET | **123★**，创建 2026-07-01，2026-09-14 更新，MIT | 盯 Claude Code | 随 agent 状态变表情、气泡、一键授权、token 用量/花费统计 | 本地优先 |
| **Copiwaifu** | github.com/Panzer-Jack/Copiwaifu | **45★**，创建 2026-04-04，2026-05-26 更新，MIT | Claude Code、GitHub Copilot、Codex、Gemini CLI、OpenCode（往各 CLI 配置注入 hook） | **Live2D 角色**（Vue3 + Tauri 2 + PixiJS 8 + easy-live2d），可换 Live2D 模型；AI 对话可接 OpenAI/Anthropic/DeepSeek/Qwen/Kimi/GLM/豆包/千帆/混元/MiniMax | 仅 macOS。二手资料称内置模型名「Yulia」（未一手核实） |
| **Vibe Pet**（Seeed Studio） | github.com/Seeed-Solution/vibe-pet | **50★**，创建 2026-06-12，2026-07-15 更新，MIT | **15 种**：Codex、Cursor、Windsurf、Claude CLI、Claude Code、Gemini CLI、Copilot CLI、CodeBuddy、Kimi Code CLI、Qwen Code、OpenClaw、opencode、Qoder、Hermes Agent、Reasonix CLI | 角色来自 **Petdex** 画廊（「thousands of characters」）或自制；⭐ **每个 agent 一张独立宠物卡**（非合并状态）；桌面可同时多只、各自可拖拽；**可 BLE 同步到 ESP32-S3 / Wio Terminal 硬件** | 硬件厂背景；「一眼看 token 与任务状态」 |
| **all-pet** | github.com/haverainlilili/all-pet | **3★**（很小，2026-09-05 创建，高频更新） | **9 平台**：Codex、Claude/Claude Desktop、DSH、Grok、Cursor、WorkBuddy、Qoder、pi、Z Code | **一只宠物看九个平台**（同一只）；可安装标准宠物包，macOS 还能导入 **Codex / OpenPets / cc-haha / clawd-on-desk / LingChat** 的宠物；自带「宠物生成标准包」（角色设计指南 + 各状态提示词模板 + 8×11 图集打包脚本） | 规模小但**接口/宠物包格式最开放**，说明「宠物包互通」已成社区惯例 |

### 2.3 长尾（2026 年密集出现，多为单人项目）

| 名称 | star | 一句话 | 值得注意的点 |
|---|---|---|---|
| UniPet (ydyangdan) | 39★ | Codex/Claude Code/Hermes/OpenClaw/DeepSeek-TUI + 自定义 | 「universal」定位 |
| CoPet (ChanceYu) | 35★ | Claude Code/Codex/Antigravity/OpenCode/Cursor/Copilot/Gemini | Rust/Tauri，多 agent |
| frieren-agent-monitor-pet | 27★ | macOS，Claude Code/Codex/Cursor/Pi + 远程 SSH | 用了《葬送的芙莉莲》IP 意象 |
| Rallo (Eyakub) | 20★ | macOS，笔记提醒 + agent 需要你时招手 | CLI 优先 |
| Aivatar (ruiwuniu) | 13★ | Codex/Claude Code/opencode/自定义 bridge → **像素房间**伴侣，可选角色 | 偏「房间养成」 |
| clawdi (can1357) | 10★ | 原生 macOS（AppKit/CoreAnimation），Claude Code/Cursor | 原生实现路线 |
| pixel-agent-buddy (44-99) | 7★ | Claude Code + Codex，**明确不做权限接管**（隐私优先） | 隐私定位 |
| AgentCat (LRainner) | 4★ | Rust 轻量，动画宠物 + 实时状态 | 2026-10-09 当天仍在更新 |
| coding-agent-pet (golitter) | 4★ | Claude Code/Codex/OpenCode hooks，随生命周期切动画与台词；默认角色「南琴梨」 | **IP 换皮**路线 |
| miku-on-desktop | 0★ | 初音未来桌宠 + **自身就是 Agent**（可截图、操作键鼠、MCP、记忆），通过 ACP 委派给 Claude Code/Codex | 反向路线：宠物是主人，agent 是外援 |
| pet-tty (Wanbinyu) | 1★ | Windows，本地 hook + 本地 bridge + 多会话 | 对应中文博客里的「PetDeck」 |
| tokengotchi / varun-ml/buddy / chencheng-li/codex-desktop-buddy | 1–8★ | 电子鸡式/实体桌宠（BLE） | 实体硬件方向 |
| **DSH 生态**（同一平台下的众多宠物插件） | — | `dsh-pet`(1184★)、`dsh-pet-indesktop`(822★)、`dsh-whale-musume`(118★)、`dsh-kun-like-pet`(105★)、`dsh-token-pet`(56★)、`dsh-jingling`(54★)、`dsh-pet-remielle`(51★)、`dsh-personal-center`(118★) 等 | ⭐ **一个 agent 平台（DeepSeek Harness）已经长出一个完整的「桌宠插件市场」**，各插件自带原创/二创角色（鲸鱼娘、鲸灵、坤…），素材沿用 Codex 精灵图契约（8 列×9 行，192×208） |

### 2.4 非 GitHub / 商业与官方产品（重要）

| 产品 | 来源 | 关键信息 | 可信度 |
|---|---|---|---|
| ⭐ **Codex Pets（OpenAI 官方）** | OpenAI Codex 应用，2026-05 上线 | **官方功能**：Codex 桌面 app 的浮动桌宠，显示 running / waiting for input / ready for review，可点击回复；`Settings > Appearance > Pets`，`/pet` 开关；**8 只内置像素宠物**（cat、dog、Codex、Dewey、Fireball、Rocky、Seedy、Stacky、BSOD、Null Signal）；`/hatch` 技能可用文字/图片**生成自制宠物**并本地保存共享；社区出现 PetShare / PetDex 等分享站 | 多家媒体一致（PCMag、ExtremeTech、Mashable、DigitalTrends、IT之家、搜狐）——**二手但多源一致**；未直连官网核实 |
| **Claude Desktop Buddy（Anthropic 官方）** | github.com/anthropics/claude-desktop-buddy | Anthropic **官方开源参考项目**（Claude Hardware Interface / Bluetooth API）：把 ESP32-S3（~$30 M5StickC Plus）变成实体桌宠，镜像 Claude Code 活动，**可按 A 键批准 / B 键拒绝权限请求**；内置 18 只 ASCII 宠物×7 种动画；可拖入自制 GIF 角色包（Bufo 青蛙为默认）；协议文档 `REFERENCE.md` 公开（Nordic UART Service） | 一手仓库存在；细节来自 CNX Software / Espressif / 虎嗅等——二手 |
| **DeskFamiliar（Steam 商业产品）** | store.steampowered.com/app/4943020 · DeskFamiliar/deskfamiliar-agents-bridges (MIT, 2026-07-15 创建) | 发行商 LinxTech，**2026-09-22 上架**，Free to Play + 角色 DLC，Windows/Godot，14 语言。3D 桌宠「外壳」，通过**开源 bridge 层**接 Codex / Claude Code；内置基于 Hermes Agent 的自家 agent；角色含**牛顿、爱因斯坦、原创角色、动物**；造型/服装/场景/特效为付费内容；Steam 披露使用了生成式 AI 制作部分资产 | 一手 bridge 仓库已核实（★1, MIT）；Steam 细节二手 |
| **智码 AICoder 桌面宠物** | aicoder.ruoyi.plus/features/pet.html | 国产 AI 编程 IDE 的**内置**桌宠：跟随当前会话状态变表情（倾听/思考/回话/报错），**身份/等级/状态三槽位分离**（状态只做局部点缀，不覆盖角色主色，仅报错整只变色）；支持**导入自定义角色包**；多开时每个实例带识别角标；v5.4.0 重做 | 官网 + B 站演示——二手，未直连 |
| **MiniCPM 官方 README 背书** | OpenBMB/MiniCPM | 官方 README 提及桌宠支持 Cursor / Claude Code / Codex | 二手 |

---

## 3. 直接回答五个问题

### Q1. 基线项目是否还在？star？支持什么？形象机制？是否还在更新？

在，但**分化明显**（详见第 1 节表）：

- **仍然活跃且是第一梯队**：clawd-on-desk（6,400★，当天提交）、agentpet（375★，当天提交）、OpenPets（1,274★，当天提交）、dsh-pet 系列（1,184★ / 822★，当天/三天内）。
- **停滞**：oc-claw（355★，4 个月未动）、Hopet（63★，2.5 个月）、aemeath_withclaude（192★，4.5 个月）、clawd-conduit（180★，创建当天推完即停）。
- **已死/改向**：Agentic-Desktop-Pet（338★，9 个月未动）。
- **查无**：claw-universe/desktop-pet-notify。
- **基线遗漏的大玩家**：cc-haha（14,932★）、petdex（4,215★）、super-agent-party（2,723★）、HermesPet（598★）、MiniCPM-Desk-Pet（492★）。

形象机制上，**基线项目的共同点是「可换形象」而非「随 agent 变形象」**：clawd 是主题 + 可导入 Codex Pet 包；Hopet 是 Hopi 主题 + 可导入；agentpet 是宠物画廊 + 按项目分配；OpenPets 是 Pet Gallery + 按 agent 窗口分配。

### Q2. 2025–2026 新入局者有哪些？

见第 2 节。**最值得注意的三件事**：

1. **平台方亲自下场**：OpenAI 在 2026-05 给 Codex 上线官方桌宠（Codex Pets），Anthropic 在 2026-04 开源了硬件桌宠 Claude Desktop Buddy，DeepSeek Harness 周边长出了桌宠插件市场，国产智码 AICoder 内置桌宠。**桌宠从「社区整活」变成了 agent 产品的标配特性**。
2. **出现了「宠物分发层」**：petdex（4.2k★）做宠物画廊 + 包格式 + 桌面 app + API，21 个项目构建在其上；all-pet 能导入 5 种不同项目的宠物包；Codex 的 8×9 精灵图契约事实上成为社区标准。**换形象这件事已被基础设施化**。
3. **短短半年 40+ 个项目入场**，且 2026-06 之后新项目的同质化极高（都是 hook → 状态机 → 12 个左右动画状态 → 气泡 → 权限审批）。

### Q3. 支持多个 agent 工具的有哪些？支持到什么程度？

**支持 10 种以上（广度型）**：

| 项目 | agent 数 | 深度差异 |
|---|---|---|
| **clawd-on-desk** | **27 种**（README 列名） | 分级：Claude Code / Codex 开箱即用（含**权限气泡**）；Copilot CLI / Gemini / Cursor / Qwen / Kimi 等需在设置里开；CodeBuddy 支持 HTTP 权限 hook；Grok / WorkBuddy / Antigravity **仅状态无权限**；另有**自定义 HTTP agent** 接口（v1 仅状态） |
| **Vibe Pet** | 15 种 | 自动安装/同步各 agent 集成；每个 agent 一张宠物卡 |
| **agentpet** | 11 种 | 一键装 hook 或 `agentpet run -- <cmd>` 包装任意命令；Codex/Claude 报「工作中/等待输入/完成」，Cursor/Windsurf/Antigravity/Copilot/Kiro/Pi 只报「工作中/完成」 |
| **all-pet** | 9 种 | Cursor/Qoder 有 opt-in observer hook；其余读本地状态文件；**任务唤醒**能力分平台降级 |
| **oc-claw** | 7 种 | 本地 hook/插件 + 远程 SSH 到 OpenClaw/Hermes 实例 |
| **CoPet / UniPet / Copiwaifu / LLMPET / Petdex** | 5–7 种 | 以 hook 为主，能力层次较浅（状态 + 气泡，部分有权限） |
| **Vibe Pet / petdex / OpenPets** | 5–15 种 | 多走「插件/MCP/包格式」路线 |

**支持 1–4 种（深度型）**：Hopet（Claude Code + Codex CLI + 两个 VS Code 扩展，但**权限与提问可在气泡/灵动岛内直接答**，做得最深）、cc-haha（Claude Code 为核心的完整工作台）、Coding-agent-pet、frieren 等。

**结论**：多 agent 支持在头部项目里是**标准配置**，不是差异点；真正的差异化在于**权限审批的深度**（能否从桌面直接 Allow/Deny）与**跨 agent 的会话统一视图**。

### Q4. 有原创 / 自带角色（而非泛用换皮容器）的有哪些？

**自研原创角色**：
- **clawd-on-desk**：Clawd 像素蟹（原创）、Calico 三花猫、Cloudling 云宝，另有官方主题 Hash Sage（哈希仙人）、Whale-chan（鲸鱼娘）
- **cc-haha**：搭搭、弧弧、补补、回回（4 个自研角色）
- **Hopet**：Hopi 像素小海豹
- **dsh-pet**：蓝色大肥鱼；dsh-whale-musume 的鲸鱼娘；dsh-jingling 的鲸灵
- **OpenBMB MiniCPM-Desk-Pet**：neko 风格猫娘 + LoRA 人格
- **DeskFamiliar（Steam）**：原创角色 + 牛顿/爱因斯坦 + 动物（3D，付费 DLC）
- **智码 AICoder**：内置角色 + 自定义角色包
- **oc-claw**：README 的「OC = Original Character」口号，但实际形象是自绘像素角色 + 用户自制

**用现成 IP 换皮的**（说明「换皮容器」路线已经很多）：aemeath_withclaude（鸣潮 爱弥斯）、coding-agent-pet（南琴梨）、miku-on-desktop（初音未来）、dsh-kun-like-pet（坤梗）、frieren-agent-monitor-pet（芙莉莲意象）、super-agent-party（neuro-sama 式 / Live2D / VRM）。

**纯容器（不预设角色，全靠导入）**：petdex（画廊 + CLI + 桌面 app，本身不拥有角色）、OpenPets（Pet Gallery + 插件 SDK）、alterhq/openpets、X-T-E-R/OpenPet、all-pet（自带 5 只但定位是容器）。

⚠️ **对我们的含义**：原创角色本身**不足以构成壁垒**——头部项目全都在做原创角色，而且已经有 petdex 这样的公共画廊把「角色」变成了可下载商品；Codex 的 `/hatch` 让用户用一句话生成自己的宠物。原始角色设计的价值只能来自**角色与产品的绑定深度**（性格、行为、长期陪伴感），不能来自「我们有原创角色」这个事实。

### Q5. ⭐ 有没有任何一个做到「形象随所运行的 agent 工具变化」？

**有，但形态与我们的设想不同。** 一手证据（README 原话）：

| 项目 | 原文 | 实现形态 |
|---|---|---|
| **OC-Claw** | 「Custom character animations, **pair different agents with different characters**」 | 不同 agent 配不同角色（多角色 + 映射配置） |
| **OpenPets** | 「**configure which pet monitors each workspace or agent window** from the built-in Pet Gallery」 | 每个 agent 窗口/工作区可指定不同宠物 |
| **HermesPet** | 「**桌宠会跟随当前 AI 模式切换**」 | 切换 AI 终端（Cloud/Hermes/OpenClaw/Claude Code/Codex/QwenCode/Kimi）时换桌宠 |
| **Vibe Pet** | 「every active editor or AI coding agent gets **its own pet card** instead of sharing one combined state」 | 每个 agent 一张独立宠物卡（是否每卡不同角色未证实） |

**本次未找到的**（这是唯一还算空的位置）：

- **没有发现任何一个做「同一个角色在不同 agent 下呈现不同面貌/姿态/皮肤」**。上面四个全是「**换角色**」（agent A → 角色甲，agent B → 角色乙），属于**多角色映射**；而不是「**同一角色的多形态**」（同一个角色，遇到 Claude Code 时戴安全帽、遇到 Codex 时换另一套动作/装扮）。
- 也没有发现把「形象随 agent 变化」作为**产品核心定位/主打宣传语**的项目——clawd-on-desk、agentpet、petdex 等头部一律是「一个角色看所有 agent」，per-agent 换形象在这些项目里最多是**配置项**。
- 也没有发现「**自动**」推断的迹象（现有都是用户手动指定映射）。

**判读**：我们设想中的「最独特卖点」在**机制层面已被实现过**（4 个项目），在**表达层面（同一角色的差异化形态）尚无人做**，在**定位层面（以此为核心卖点）也尚无人做**。所以它**不能算原创卖点，但可以算「有先例但无人占据的表达位置」**——这一点必须在产品决策里明确知道，避免把它当成「市面上没有」的护城河。

---

## 4. 「未找到」清单（如实记录）

1. **claw-universe/desktop-pet-notify**：仓库直查未成功（API 限流），关键词搜索（`claw-universe`、`desktop-pet-notify`、`lotosbin/claw-universe`）只返回无关或 0★ 空仓库。**基线这条未能证实**。
2. **「同一角色随 agent 变化形态」的具体实现**：未找到任何项目（见 Q5）。
3. **「自动」按 agent 切换形象（无需用户配置）**：未找到。
4. **通义灵码 桌宠**：中英文搜索均**未出现**「通义灵码」相关桌宠项目；与之最接近的只有 clawd-on-desk 支持列表里的 **Qwen Code / QwenWork（千问办公）**，以及 PetAI 支持「通义」模型接口。
5. **Trae 官方桌宠**：未找到；只有 clawd-on-desk 支持列表里的 **TraeCode (Trae CN)**。
6. **豆包 桌宠**：豆包只以「模型服务商」身份出现（PetAI、Copiwaifu 可接火山方舟/豆包），**未发现豆包作为被监控的 coding agent**。
7. **Cline / Roo Code / Aider 的桌宠**：Aider 只在 RE-LLMPET 的 provider 列表中被提及（该仓库直查 404，未能核实）；Cline / Roo Code 未找到任何桌宠项目。
8. **VS Code / JetBrains 官方或热门的「agent 状态桌宠」扩展**：未找到（找到的都是独立桌面 app，不是编辑器扩展）。
9. **把桌宠作为付费订阅主卖点的 SaaS**：未找到（商业化的只有 DeskFamiliar 的 F2P + 外观 DLC）。

---

## 5. 不确定项 / 需要进一步核实

| 事项 | 状态 | 说明 |
|---|---|---|
| **oc-claw 的许可证与当前真实状态** | 未取得 | API 限流 403；README 未提许可；官网 oc-claw.ai 不可达（本次网络限制）。README 最后更新 2026-06-17，但 GitHub 之外的发布渠道（官网下载 v1.8.x）可能仍在更新——**V2EX 帖显示 2026-05 时官网已到 v1.8.4** |
| **oc-claw「不同 agent 配不同角色」的具体交互** | 仅 README 一句话 | 未读到文档/截图级证据；是否每 agent 独立选择角色、是否支持同时显示多角色，**未证实** |
| **Vibe Pet 每张 agent 宠物卡是否可配不同角色** | 未证实 | README 只说「own pet card」；character switching 与 per-agent view 分属两个 bullet，**两者是否联动未证实** |
| **HermesPet 的 5 只内置桌宠名单（Clawd、云朵、fomo、Pegasus、coco）** | 未证实 | 该列表来自 CSDN 转载的推荐文；README 只写「桌宠会跟随当前 AI 模式切换」 |
| **HermesPet 许可证** | 冲突 | GitHub API 返回 license = null（公开仓库无 LICENSE 文件，仓库以发布/介绍为主）；二手来源称 Apache-2.0。**以 null 为准** |
| **Codex Pets 的官方细节**（8 只内置、`/hatch`、`/pet`、Cmd+K 开关） | 二手多源一致 | 未直连 OpenAI 官网核实（网络限制），但 PCMag / ExtremeTech / Mashable / DigitalTrends / IT之家 / 搜狐 说法一致 |
| **DeskFamiliar 的 Steam 细节**（2026-09-22 发售、F2P + DLC、Godot） | 二手 | Steam 不可达；bridge 仓库 `DeskFamiliar/deskfamiliar-agents-bridges`（★1，MIT，2026-07-15 创建）已一手核实存在 |
| **RE-LLMPET（purrfecto114-lgtm/RE-LLMPET）** | 直查 404 | 搜索结果称其支持 5 个 agent + 跨 agent 交接（Claude ↔ Codex 双向），但**仓库直查返回 404**，可能改名/转私有/删库。**不予采信** |
| **多个 stars 数的时间差异** | 已标注 | 中文博客（腾讯云、CSDN）在 2026-09 引用 clawd-on-desk 为 2.6k★，本次为 6.4k★——**半年内增长约 2.5 倍**，说明该赛道热度仍在快速上升 |
| **npm 生态项目（companion-mcp、@ag9898/buddy、claude-session-pet、@shigureni/minion、@clawmate/clawmate、@asahimoon/dsh-desktop-pet）** | 仅搜索线索 | 未做一手核实（npm 站点本次不可达）。其中 **ClawMate** 被描述为「多角色切换 + 多 agent 场景下按 Agent 单独配置」，若属实则**又是一个 per-agent 形象项目**，建议后续核实 |
| **cc-haha 的 8,603 fork** | 存疑 | fork 数异常高（接近 star 的 58%），未核实是否为刷量或 fork 活动；统计时建议只用 star |

---

## 6. 给产品决策的几点直接提示

1. **「多 agent hook 接口」不构成差异化**——头部项目已经做到 27 种，且已有自定义 HTTP agent 接口（clawd-on-desk）与 MCP 通用层（OpenPets）。建议把它当作**及格线**而不是卖点。
2. **「形象随 agent 变化」有 4 个先例，但没有一个把它当核心定位**。若坚持这条路，唯一还没人占的表达是「**同一原创角色的多形态**」，并且需要回答：用户为什么会想要「同一个角色换装扮」而不是「给每个 agent 配一只不同宠物」——后者已被实现且更符合直觉（每个 agent 有自己的宠物）。
3. **真正的空白位（本次调研中头部项目做得都不够的）**：
   - **权限审批的完整闭环 + 深度**：目前只有 clawd-on-desk / Hopet / oc-claw / Claude Desktop Buddy（硬件）做到桌面直接 Allow/Deny，且各有限制（有的只支持 2 个 agent，有的只支持状态）。
   - **跨 agent 的统一会话视图 + 一键跳回对应终端的精确会话**：all-pet 明确承认多处「fail-closed / 平台降级」，说明这是普遍难点。
   - **陪伴感（长期记忆、性格演化）**：clawd-on-desk 完全没有；做这个的（agentpet 养成、buddy 评审、MiniCPM 本地模型人格、arona-agent）都还不成熟。
   - ⭐ **「同一角色多形态」+「陪伴感」的组合**是本次调研里**确实没有对手**的位置。
4. **注意平台方风险**：OpenAI / Anthropic / DeepSeek / 国产 IDE 都已亲自下场。**单点功能（比如「Claude Code 状态桌宠」）随时可能被平台原生能力吃掉**，产品价值必须建立在平台不会做的事情上（跨平台、跨 agent、原创 IP 的长期陪伴）。cc-haha 与 MiniCPM-Desk-Pet 都直接基于 clawd-on-desk 二次开发，说明**这个赛道的技术壁垒极低**。
5. **宠物素材已有公共基础设施**：petdex 的包格式 + Codex 的 8×9 精灵图契约 + all-pet 的宠物生成标准包，意味着自研角色可以走**兼容格式**（降低用户迁移成本，也便于将来被别的工具导入）——但反过来，**用户也可以随时把我们的角色换成别人的**。

---

## 附录 A：本次调研的一手数据采集命令（可复现）

```bash
# 单个仓库元数据（star / 创建 / 最近提交 / 许可证）
curl -s https://api.github.com/repos/<owner>/<repo>

# 关键词搜索（独立配额：10 次/分钟，与上面 60 次/小时的配额分开）
curl -s "https://api.github.com/search/repositories?q=<kw>&sort=stars&order=desc&per_page=20"

# 读取 README 原文（base64）
curl -s https://api.github.com/repos/<owner>/<repo>/readme
```

注意：`raw.githubusercontent.com` 与 `github.com` 网页在本次会话不可直连，只能走 `api.github.com`。

## 附录 B：本次一手核实的仓库清单（star 均为 2026-10-09）

clawd-on-desk 6400 · cc-haha 14932 · petdex 4215 · super-agent-party 2723 · OpenPets 1274 · dsh-pet 1184 · dsh-pet-indesktop 822 · HermesPet 598 · MiniCPM-Desk-Pet 492 · agentpet 375 · oc-claw 355 · Agentic-Desktop-Pet 338 · aemeath_withclaude 192 · clawd-conduit 180 · buddy(fiorastudio) 115 · LLMPET 123 · alterhq/openpets 100 · Hopet 63 · X-T-E-R/OpenPet 57 · vibe-pet 50 · Copiwaifu 45 · clawd-insights 43 · UniPet 39 · CoPet 35 · codepet 29 · frieren-agent-monitor-pet 27 · Rallo 20 · Aivatar 13 · agent-pulse 11 · clawdi 10 · pixel-agent-buddy 7 · coding-agent-pet 4 · AgentCat 4 · all-pet 3 · DeskFamiliar bridges 1 · pet-tty 1 · varun-ml/buddy 8 · miku-on-desktop 0（另有 Docker 镜像/官网等非 GitHub 来源）
