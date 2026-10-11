# B 路调研：AI agent 状态指示器 + 「形象随应用 / AI 状态差异化」机制先例

- 调研日期：**2026-10-09**
- 调研范围：本文只回答两件事
  - **(a)** 面向 AI 编码 agent 的状态指示 / 通知类产品（**非桌宠形态**）：菜单栏 / 托盘 / 悬浮窗 / 硬件灯 / 编辑器内 / 终端状态栏
  - **(b)** 「同一个形象 / 角色随所运行的应用或 AI 状态做差异化变化」的机制先例（含硬件外设 app-aware、桌宠换装、AI 伴侣状态头像、付费皮肤生态）
- 不在本文范围：Claude Code 桌宠类产品本身（clawd-on-desk / clawd-conduit / agentpet / openpets / oc-claw 等，属另一路）

---

## 0. 调研方法与可信度说明（重要）

1. **本次 WebFetch 被网络策略全面拦截**（github.com、raw.githubusercontent.com、npmjs.com、docs.claude.com、甚至个人 github.io 全部返回 "Unable to verify if domain … is safe to fetch"），Bash 侧 curl 也不通（连接重置）。因此**所有信息均来自 WebSearch（搜索引擎侧抓取页面后返回的摘要）**，我**无法打开原始页面逐字复核**。
2. 对每条关键结论，我尽量要求**多个独立来源交叉一致**。凡是单一来源 / 二手转述的，已在下文和 §6 明确标注「单一来源」「未证实」。
3. **星级数字一律标注为近似值并注明测量来源与日期**，因为无法直连 GitHub 复核。凡搜索摘要未给出确切星数的，我**没有编造**。
4. 时间基准 2026-10；凡涉及 2025 年之前的数据均注明日期。
5. 中文、英文、日文关键词均已尝试（见 §7 关键词清单）。

---

## 1. 结论速览（TL;DR）

### 1.1 对 (a) 的直接回答

**这类产品在 2025–2026 已经形成密集赛道，且竞争激烈。** 至少可分成 7 个形态大类：

| 形态 | 代表 | 数量级 |
|---|---|---|
| macOS 菜单栏 / 刘海 | claude-island、claude-status-bar、Agent Signals、CC Lights、Agent Signal Bar、菜单栏 CLI 状态监控（App Store） | 极多 |
| Windows 任务栏 / 托盘 | AgentTray、SlopBar、ai-taskbar-widget、Gengchou、Notchcode（也有 mac 版） | 多 |
| 桌面悬浮窗 / 软件红绿灯 | AI Light、QuotaBubble、Agent Signal Bar（含悬浮态） | 中 |
| **物理状态灯（硬件）** | vibe-lamp（ESP32）、cclight、agent-traffic-light-monitor、icefoxj/claude-monitor（M5Stack）、VibeSignal（Luxafor）、Copilot Buddy（蓝牙） | 已成一个独立小品类，GitHub `status-light` topic 下 23+ 仓库 |
| 编辑器内（VS Code / JetBrains） | Claude Pulse（VS Code 状态栏 + Buddy 角色面板）、Pixel Agents、Star UI、Agent Arcade、copilot-agent-pets | 多 |
| 终端 statusline | ccusage statusline、cc-usage-bar、ccsidekick、ccpet、ccfeline、claude-lantern、clawde | 很多 |
| 用量 / 额度监控 | ccusage 及一大批菜单栏/任务栏封装（cc-usage、ccusage-monitor、cctray、CCOwl、claude-analyst、SlopBar…） | 极多 |

**「有没有用角色 / 形象来表达状态的？」→ 有，而且已经是成熟子类，至少 10+ 个产品：**

- 终端 / 状态栏：`ccsidekick`（可反应角色 + 主题，MIT）、`ccpet`（虚拟宠物，能量随 token 消耗，颜文字表情分级 `(^_^)/(o_o)/(u_u)/(x_x)`）、`ccfeline`(`@sebryu/cccat`)（猫，会睡觉/呼噜/走过提示符）、`claude-lantern`（绿灯侠主题，切换角色改变配色、旋转器、mascot）、`clawde`（状态栏宠物，皮肤 buddy/cat/slime + 小游戏）、`clawd-buddy`
- VS Code：**Claude Pulse**（`zxrrcpandey/VSCode-ClaudeCode-Status-Bar`，状态栏 + **Buddy 角色面板**随 Claude 状态反应 + 子代理计数 `⟳ Claude · working 4:46 · 3 agents` + 本地 token 统计）、**`claude-companion-app`**（侧边栏动画 mascot + 工具活动流）、**Pixel Agents**（像素办公室，**一个 agent = 一个角色**，6 个不同角色，打字/阅读/走动/气泡）、**Star UI for Claude Code**（活动栏 Office 视图，角色走到对应房间）、**Agent Arcade**（点击角色可看任务摘要）
- 刘海：**TokenNotch**（MacBook 刘海 widget，用**官方 Clawd 与 Codex 两个不同吉祥物**显示剩余额度 + HP 条 + 重置倒计时）

**「有没有做到『显示 agent 正在做什么』（任务摘要）或『就地审批』的？」→ 两者都有成熟实现：**

- 任务摘要：Claude Pulse（任务进度 + 子代理数）、Claude Companion（工具活动 feed：idle/thinking/running tools/writing files/errors）、claude-island（live session + 聊天历史）、Notchcode（会话详情）、Agent Arcade（点击角色看 model/branch/task summary）、AI Light（按 git 根目录做项目级聚合）、Claude Pulse 的 token 本地计算
- 就地审批：**claude-island**（刘海内批准/拒绝 + 聊天历史，Unix socket 通信）、**Hopet**（权限请求原位展开为 Allow/Deny/交给终端卡片）、**LLMPET**（桌宠上一键允许/拒绝）、**Copilot Buddy**（物理按钮批准/停止）、**clawd-on-desk**（Ctrl+Shift+Y/N，属另一路已知基线）

> ⚠️ **必须纳入判断的 2026 新变量：第一方官方已经入场。**
> - **OpenAI「Codex Pets」**（2026-05-01/02 上线，Win+macOS）：`/pet` 唤醒，8 个内置宠物 + `/hatch` 用图片自定义宠物；浮在桌面最上层，气泡显示 **Running / Needs input / Ready / Blocked**（优先级 Needs input > Blocked > Ready > Running），**点宠物可直接回消息给 agent**；自定义宠物消耗 Codex 额度；上线即被社区做成分享站（Clippy、Sam Altman、宝可梦等）。来源：Mashable、PCMag、Digital Trends、Gigazine、IT之家、unwire。**这是官方第一方、形态几乎完全同构的产品。**
> - **Anthropic「Claude Code Buddy」**（`/buddy`，v2.1.89，源头是 2026-04-01 愚人节彩蛋，盐值 `friend-2026-401`）：终端输入框旁的 ASCII 精灵，**18 个物种 / 5 个稀有度 / 1% Shiny / 6 种眼睛 / 8 种帽子 / 5 项属性（DEBUGGING、PATIENCE、CHAOS、WISDOM、SNARK）**，由账户 UUID 确定性生成，`/buddy pet` 触发爱心动画。来源：Futurism、GitHub issue #67929、claude-code-best 镜像文档、腾讯云技术文章。**但「v2.1.97 前端被移除」的说法仅见于单一 GitHub issue，未证实。**
> - **GitHub Copilot CLI 自带 mascot**：changelog v1.0.15（2026-04-01）明确写「Copilot mascot now blinks with subtle eye animations in interactive mode」。
> - **Anthropic 官方吉祥物 Clawd**（像素橘色螃蟹，CLI 内以四分之一方块字符绘制的 sprite，**4 个姿势**，色值 rgb(215,119,87)），开发活动发实体周边。
> - 微软 **Mico**（Copilot，2025-10-23 发布）：团状角色，**会移动、说话、笑、跳舞、变形、变色，被形容为「像电脑的情绪戒指」**，可换外观与声音，反向支持「Learn Live」变成戴眼镜的导师形象。

### 1.2 对 (b) 的直接回答

**「按应用 / 上下文自动切换外观」这个机制在业界极其常见，绝不是新概念——它几乎是被每一家外设厂商和操作系统厂商做成了标配。**

**① 硬件外设 app-aware（普及度最高的类比）——「按应用切换外观」是行业标准功能：**

| 厂商 / 产品 | 机制 | 关键细节 |
|---|---|---|
| **Razer Synapse / Chroma** | 在 Synapse「Profiles」里把 Chroma 灯光配置文件 `+` 绑定到某个 `.exe`，程序获得焦点时自动切换，失焦回默认 | 可绑非游戏程序（DAW、编辑器）；**必须在 Synapse 运行时才生效**；另有 Chroma Connect（官方应用集成开关）与 **Chroma Immersive「Screen Sync」——分析前台窗口画面实时驱动灯效，无需游戏做集成**。⚠️ Synapse 3 自 **2026-02-03** 起停止更新与云同步，需迁移 Synapse 4（Razer 官方支持文号 1940） |
| **Logitech G HUB / LGS** | **「自动游戏检测」**（鼠标上的锁图标处于解锁态即启用）+ 手动为 profile 绑定进程名（如 `Photoshop.exe`） | **该功能默认关闭**；匹配到未配置的 profile 会回落到默认设置——是常见坑；支持「锁定 profile 在游戏运行时」；第三方 FAQ 明确「另一个程序抢前台会回落到默认」 |
| **Corsair iCUE** | iCUE 4+：profile → Edit Profile → **Linked Applications** → 添加 `.exe`；iCUE 3 及更早：**"Link profile to program"** | 还有 SDK 层：Game SDK 开关 + Application Control / Device Control 白名单；Game State Mode 需要按游戏单独配 profile |
| **Elgato Stream Deck** | **Smart Profiles**：Profiles 标签页里给 profile 指定 Application，前台切到该应用即自动切换 | 官方文档：**不支持 UWP 应用**；**Stream Deck 软件必须最小化**（打开时 Smart Profiles 失效，防止编辑时乱切）；Marketplace 有大量预制 profile |
| 第三方 | **Keylegend**（GitHub）：Razer Chroma 的交互式键盘灯，**约 90 个应用 profile**（Photoshop、VS Code、Excel、Elden Ring…），应用获得焦点时激活 | 演示了「app → 一套完整视觉配置」的规模化 |

> **结论**：「按应用换外观」在外设领域是 2010 年代就成熟的标配能力，同时被 Razer / Logitech / Corsair / Elgato 四家独立实现。**这说明用户对这种交互范式有明确的既有心智，不是要教育的新概念。** 但同时说明：**这个机制本身完全没有新意**，不能作为卖点主张，只能作为「用户已经理解」的既定交互。

**② 操作系统级 context-aware 外观（最大规模的先例）——**

- **Apple Focus 模式 → 自动切换 Apple Watch 表盘 + iPhone 锁屏 + 主屏**（watchOS 8 起，watchOS 9+ 深化，iOS 16 起锁屏/主屏也加入）。关键的是：**Focus 可以由「使用某 App 时」自动触发**，还有 **Smart Activation** 按位置与 App 使用习惯预测。来源：Apple 官方支持文档 `Use Focus on Apple Watch`。
  → 这是**消费级、十亿量级装机、由第一方推的「同一设备随上下文换整套外观」**，直接证明该模式的用户接受度。

**③ 桌宠 / 虚拟形象 app-aware 的先例（比我预期的多）：**

| 产品 | app-aware 机制 |
|---|---|
| **MateEngine**（Steam 3625270，2025-04-16 上线） | VRM 3D 桌宠。**「Danceability」白名单**：把 Spotify / Chrome / Firefox 等加进音频源白名单后，那些应用出声时角色就跳舞。直接从 Windows API 监听，官方/社区称不会触发反作弊；但任务栏/窗口坐姿功能因反作弊风险已禁用 |
| **MMDEngine**（Steam 2241940） | 坐在任务栏上，**实时检测本地音乐 / 在线视频 / 游戏 BGM** 并跟着跳 |
| **HearHen**（Steam 4341080，2026-02 免费） | 对**任何 PC 音频**实时反应，软音乐摇摆、强节奏跳舞 |
| **NekoAI**（Tauri+Rust 个人项目） | `desktop_monitor.rs` 用 **Win32 API 读活动窗口 + 空闲时间**，宠物在你的窗口间游荡、对你在做什么有反应，会主动提醒「coding 90 min — take a break!」 |
| **Shimeji 系（含 Linux 叉件）** | `titles.conf` 里逐行填窗口标题（大小写不敏感），**宠物只与这些窗口交互**；有「窗口层级检测」与窗口状态/类型处理的完整重写；中文版描述里宠物会爬到已开窗口上沿、窗口最小化时掉落 |
| **Shijima（iOS 越狱）** | 可**逐 App 控制桌宠是否出现** |
| **Degu Desktop** | 走在任务栏/Dock 上方，对打字和点击有反应 |
| **Lively Wallpaper / iWall** | **前台进程 / 全屏 / D3D 游戏**检测 → 暂停动态壁纸（性能向，非形象向） |
| **专利族** | US20240176626（壁纸）：**根据「第二应用运行状态」获取应用类型，在人形元素邻近位置显示对应标识**，还描述按步数 + 某体育 App 运行状态把角色换成「飞行姿势」；CN114296853A：动态壁纸进程加载 3D 虚拟角色，**读取设备状态与软件运行状态实时更新角色形象**，甚至从关联软件取文本/音频、在角色嘴部渲染口型；US20130069962A1：后台 App（如播客）驱动锁屏壁纸；US9256861：头像动画触发壁纸动画，壁纸选择反过来改头像的 mood/appearance |

**④ AI 助手 / 伴侣的「状态头像」机制：**

- **Microsoft Mico**（Copilot）：形状、颜色、动作随交互与情绪实时变化，可自定义外观与声音，同一角色可变成「戴眼镜 + 白板的导师」（Learn Live）、反复点击会临时变成 Clippy。
- **Meta Muse / Jolly**（2025-09-08）：毛绒 + 红脸颊虚拟角色，另有带 2 吋屏的电子宠物钥匙扣 **Muse Charm**。
- **OpenAI Dots**（2026-09-29 开发者大会）：个人 AI agent，**多款彩色拟人三角形角色**（黄梨戴眼镜的 Alfred、蓝云戴贝雷帽的 Felipe、默认绿豆 Dottie），**用户可自定义外观与名称**。
- **SynClub**（AI 聊天 App，2026-09-12 报道）：小说模式「Visual Production」——**心情变化时显示对应立绘，覆盖 20+ 种情绪**（喜、羞、怒、悲、惊…），章节切换时背景也换。
- **ReBot AI**：同一套表情系统贯通 chat / 命令 / 游戏 / TTS，100+ 外观与情绪。
- **AI AVATAR**：读用户语音与表情 → 头像实时变表情（上线 3 种情绪）。
- 学术/框架：MatesX（开源数字人，语音+语义+节奏多模态情绪 → 驱动表情与语气）、UTS/Springer HCI International 2025（21 人用户研究，情绪自适应头像显著提升体验质量与情感投入）。

**⑤ 「同一个角色随状态自动换装」——这是我找到的最接近我们卖点的机制（重要）：**

| 产品 | 机制 | 与我们卖点的差异 |
|---|---|---|
| **dsh-lived-pet**（DeepSeek Harness 桌宠，Live2D） | **换装系统**：气泡悬停展开 4 个槽位（头饰 / 眼镜 / 贴纸 / 桌面），20 个可叠加素材；**「装扮自动穿脱」——读文件时自动戴上眼镜、写代码时自动贴贴纸**；用户手动选过的槽位不被覆盖 | 触发源是 **agent 的当前状态/动作**，**不是「哪个 agent 工具」** |
| **whale-girl-plus**（DSH 插件） | 多套服饰（经典女仆 / 樱粉 / 都市职业 / 运动服）；**「自动换装」：随机抽到「打网球」休闲动作时自动换运动服，结束后自动换回**，不覆盖用户偏好 | 触发源是**随机 idle 动作** |
| **ChatWaifu**（Steam 2331610） | 角色模型支持自定义服饰与姿势，**「can be auto switched by AI」** | 描述含糊，未证实具体触发源 |
| **Claude Code Buddy / Buddy 系列** | 「pet」触发爱心动画 | 单状态动画，非换装 |

**⑥ 「每个 agent 配不同角色」——已有人做了，但做的是「不同角色」而非「同一角色不同面貌」（关键差距）：**

| 产品 | 机制 | 关键差异 |
|---|---|---|
| **OC-Claw**（`rainnoon/oc-claw`，v1.8.6，开源免费，winget 可装） | 官方 feature 列表原文：**"Custom character animations, pair different agents with different characters"**；中文描述「多 Agent 可以配对不同的角色，一眼分清谁是谁；还支持多桌宠同屏」。监控 OpenClaw / Claude Code / Codex / Cursor / OpenCode / Gemini CLI / Hermes，支持 SSH 远程 | **换个 agent = 换一个角色**，不是同一角色的变体 |
| **VibeMon**（原 `vibe-monitor`，npm） | **按工具自动映射角色**：clawd（橙，Claude Code）/ codex（藏青，Codex CLI）/ kiro（白幽灵，AWS Kiro）/ claw（红，OpenClaw）/ vibemon（紫机器人，默认兜底）；**Character Lock** 可强制锁成单一角色。2.0 改为单角色 + 跟随气泡，多项目时自动切到「你该看的那个会话」 | 同为「换 agent 换角色」。注意 2.0 blog 标题是 "One Character, Smarter Than Ever"（指单一窗口/单一实例） |
| **Vibe Pet**（Seeed-Solution，开源） | **多 agent 独立视图：每个活跃编辑器/agent 有自己的 pet card**，可独立拖到桌面；角色默认从 **Petdex** 选（4,600+ 免费动画宠物）或一键自建；兼容 Codex / Cursor / Windsurf / Claude / Gemini CLI / Copilot CLI / CodeBuddy / Kimi / Qwen / OpenClaw / opencode / Qoder / Hermes；可 BLE 同步到 Wio Terminal / ESP32-S3 | 「同工具多宠物卡」，非「同角色多变体」 |
| **Hopet**（macOS，像素风） | **多会话聚合到一只宠物**，按优先级显示最高状态（AskUser > Permission > Error > Tool > Thinking > Responding > Completed > Idle），并**高亮「是谁在找我」**；8 态状态机；可导入自定义宠物（名字 + 8 张 GIF 或 Codex pet 包） | 反向设计：**一只宠物代表全部**，不按 agent 区分形象 |
| **LLMPET**（MIT） | **dsh（DeepSeek Harness）有独立「鲸鱼女仆」皮肤，23 张动画覆盖 21 个状态**；覆盖 Claude Code / Codex / dsh；支持一键审批、跨 agent 交接 | **最接近「同一产品、不同 agent 不同皮肤」**，但是「每 agent 一套独立角色」而非「同一角色的变体」 |
| **ClaudePet**（macOS/Windows） | 季节皮肤（春樱 / 夏阳伞泳圈 / 秋枫叶围巾 / 冬）**右键手动切换**；多会话模式下每个会话显示不同颜色的宠物；Claude Desktop 开关检测 + 使用时长统计 | 皮肤切换**手动**；检测只针对 Claude 家族 |
| **AgentPet** | 按**项目**分配宠物；吃掉真实 token 与完成会话，5 阶段进化（Hatchling→Companion→Scout→Hero→Legend）+ 14 项成就 + 排行榜 | 按项目，非按工具 |
| **Pixel Agents**（VS Code，MIT，~8.6k–9.4k★） | **「One agent, one character」**——每个 Claude Code 终端生成一个角色，**6 个不同角色**；子代理以独立角色出现并用虚线连到父角色；像素办公室可编辑 | 「同工具多会话 → 多角色」 |
| **clawd-on-desk**（属另一路基线） | 三套主题（Clawd 螃蟹 / Calico 三花猫 / Cloudling 云宝）+ Codex Pet zip 动画包导入。搜索摘要**未说明主题是否按工具自动切换** → **未证实** | — |

### 1.3 一句话结论

> **「形象随状态变化」不稀缺；「形象随**所运行的 agent 工具**变化」目前无人以「同一角色的差异化变体」形式做过。**
>
> 现有产品要么是 **① 同一角色随 agent 状态换动作/换装**（dsh-lived-pet、whale-girl-plus、ClaudePet 的季节皮肤）、要么是 **② 不同 agent 换不同角色**（OC-Claw、VibeMon、Vibe Pet、LLMPET、Pixel Agents）。**「一个原创角色，在 Claude Code 下是这个面貌，在 Codex 下是另一个面貌」这条轴，我翻遍中英日文关键词都没找到有人做，也没找到有人把它当卖点宣传。**
>
> 但必须冷静：这条轴的**机制门槛极低**（外设厂商十年前就做完了），**它的价值完全取决于「用户是否真的想为不同 agent 区分形象」**，而现有产品反其道而行——Hopet 明确选择「一只宠物代表全部 agent」，VibeMon 2.0 从多窗口多角色收敛到「一个角色 + 气泡」，理由都是**减少认知负担**。这是一个需要正面回应的反向证据。

---

## 2. (a) 详细清单：AI 编码 agent 状态指示 / 通知类产品

### 2.1 macOS 菜单栏 / 刘海

| 名称 | 链接 | 它是什么 | 状态表达 | 任务摘要 | 就地审批 | 形象 |
|---|---|---|---|---|---|---|
| **claude-island** | github.com/farouqaldori/claude-island | 刘海 Dynamic Island 式通知，自动装 hook，Unix socket 通信，需 macOS 15.6+ | 刘海展开动画 | ✅ live session + 聊天历史 | ✅ 批准/拒绝 | 无（用系统刘海） |
| **claude-status-bar** | github.com/m1ckc3s/claude-status-bar | 极小菜单栏 App，无窗口无 Dock 图标，首次启动自动装 hook；`brew install --cask claude-status-bar` | 思考/跑工具时动画图标、等权限黄点、回合计时 | 状态文字可选 | ❌ | **图标动画样式可选 Spark / Unicode / Clawd / Orbit**（Clawd 即官方吉祥物） |
| **Agent Signals** | producthunt.com/products/agent-signals | 菜单栏红绿灯，读 hook 写的 JSON 状态文件，纯本地，MIT，原生 SwiftUI，已公证 | 🔴等输入 / 🟡工作 / 🟢完成 | 会话维度 | ❌ | 无 |
| **CC Lights** | global.v2ex.co/t/1226660 | 菜单栏红绿灯 + 点击跳到对应终端标签/窗口/面板（Terminal.app、iTerm2、Ghostty、cmux），自带 `cc-lights` CLI | 绿工作/黄需输入/红错误/灰无会话 | 多会话 | ❌ | 无 |
| **Agent Signal Bar** | github.com/guan-ops/Agent-Signal-Bar | 菜单栏 + **可选桌面悬浮信号灯**；自动检测 Codex（Desktop/CLI/VS Code/Xcode/IDEA）；Claude 走 hook 或自带 `agent-signal` CLI；**主题 / 声音 / 用量 / 账号视图**，无账号无后端，macOS 14+ | 红黄绿 + 权限/失败优先 | 用量视图 | ❌ | 无（但有主题） |
| **菜单栏 AI 编程助手 CLI 状态监控** | App Store id6778941425 | **不从 hook、只读磁盘 transcript 文件**推导状态（无需插件/包装器）；监控 Claude Code / Codex / Gemini CLI / Cursor / Aider / 任何写会话文件的工具；沙盒只读授权目录 | 项目/状态/耗时列表 + 原生通知 + 「打开会话」+ 待输入会话角标 | ✅ 会话检查器 | ❌ | 无。免费 + 内购（订阅或买断），macOS 14+ |
| **morinpic/Claude-Status-Bar** | github.com/morinpic/Claude-Status-Bar | ⚠️ **不是会话状态**，是 Claude **服务状态**（status.claude.com）监控，故障通知。**菜单栏图标形状随系统健康度变化**（✓ 正常 / ℹ 轻微 / ! 重大 / ✕ 严重 / ? 连接错误） | 图标形状 | 事件详情 | ❌ | 无。需 macOS 26 (Tahoe)+ |
| **Burn AI** | producthunt.com/p/burn-ai | 3.4 MB SwiftBar 插件，显示 Claude Code / Codex / Aider 在工作/等待/卡住，**外加本次会话美元花费**，读 JSONL traces，无 Electron / 无云 / 无遥测，MIT | 状态 + 花费 | ❌ | ❌ | 无 |
| **Notchcode** | github.com/billxby/notchcode | 原 MacBook 刘海监控，2026-06 v1.0.0 加 Windows 版：主屏顶部居中无边框置顶状态条（Tauri 2） | 多会话实时；混合显示 Claude/Codex 用 **CC/CD 徽标** | ✅ 会话详情 + 用量/周预算 | 权限提示时一键跳回终端 | 无 |

### 2.2 Windows 任务栏 / 托盘

| 名称 | 链接 | 它是什么 | 状态表达 | 特点 |
|---|---|---|---|---|
| **AgentTray** | github.com/sprklai/agenttray | Tauri 桌面托盘，Claude Code / Codex CLI / Gemini CLI 暂停等输入时告警 | **彩色托盘点**：黄=需输入（蜂鸣+通知）/ 红=错误 / 蓝=工作 / 绿=空闲 / 灰=离线 | hook 为主 + 进程扫描兜底；能跳回终端；状态文件在 `~/.agent-monitor/`；Linux 主平台，mac/Win 支持但测试少 |
| **AI CLI Complete Notify** | github.com/ZekerTop/ai-cli-complete-notify | Tauri 2 托盘（隐藏到托盘、开机自启、mac 可隐藏 Dock），v2.17.0 | 完成提醒，多通道：Webhook（飞书/钉钉/企微）/ Telegram / 邮件 SMTP / 桌面通知 / 声音 / TTS | hooks + 日志监听混合；智能防抖、按来源阈值、可选 AI 摘要、中英双语；Linux/mac/WSL，macOS 13.5+ DMG |
| **SlopBar** | github.com/thelioo/slopbar | 任务栏时钟旁 widget，圆环显示 Claude Code 与 Codex 用量 | 圆环 + 悬停数值 | 点击展开各账号会话/每周限额、重置时间；**额度用尽自动切换账号**；Win+WSL；随系统深浅色；全屏自动隐藏；10 语言 |
| **ai-taskbar-widget** | github.com/KimJinWooDa/ai-taskbar-widget | 任务栏 widget，会话/每周/模型级剩余额度 + 重置时间 + 颜色标注风险 | 颜色 + 徽标 | 「例行任务提醒」未读徽章、技能使用统计；Codex 走官方用量 API；本地优先；双击 `install.cmd` 一键装；**韩语界面** |
| **Gengchou** | winget `ynjmxn.Gengchou` v2.4.0 | 原生 Windows 任务栏 widget + 托盘 App，监控 Claude / Codex / Google Antigravity 额度 | 数值 | 从运行中的 WSL 发行版读凭据；无遥测不存 token；无官方仓库描述（winget 清单） |
| **QuotaBubble** | pypi.org/project/quotabubble | 无边框悬浮气泡显示用量，悬停浮现、点击展开详情（含重置倒计时），可跨显示器拖动；托盘图标控制 | 气泡 + 倒计时 | 支持 Claude Code / Codex / Antigravity / GitHub Copilot / DeepSeek / OpenRouter；**Windows 为主要平台** |
| **AI Light** | github.com/LeoKemp223/ai-light | Tauri 桌面悬浮**红绿灯**窗口，四态 | 空闲 / 开发中（黄灯脉动）/ 错误需介入（红）/ 完成（绿） | **按 git 根目录做项目级聚合**；Claude 走 hook，Codex 走会话文件监听 + 进程扫描；Win/mac 桌面，Linux 仅转发 hook |
| **@erica_s/claude-code-notify** | npm | 原生 Windows Toast + **任务栏闪烁**，通知带 "Open" 按钮激活目标终端窗口 | Toast | Codex 审批提醒走 `codex-session-watch`；Win 10/11 + Node ≥16；最后更新 2026-03-31 |
| **job-finish** | npm | Windows 通知：完成 / 等待输入(AskUserQuestion) / 用量超限 / API 错误（异常停止单独标题） | Toast + 任务栏闪烁 30s/5min/10min/无限 | **焦点感知**（已在 VS Code 则跳过）；`npx job-finish init zh` 中文向导；仅 Windows、仅 VS Code 场景 |

### 2.3 物理状态灯（硬件）——独立小品类

| 名称 | 链接 | 硬件 | 状态映射 |
|---|---|---|---|
| **vibe-lamp** | github.com/laofahai/vibe-lamp | ESP32-C3 控制板 + WS2812B（含 KiCad / Gerber / BOM/CPL），也有面包板路径 | 蓝=工作 / 绿=完成 / 红=需要你 / 琥珀=断连；状态 `idle/working/done/error/needs_you`，优先级 `needs_you > error > working > done > idle`；决策全在 Mac，ESP32 只做显示端点，心跳停了进琥珀「lost」模式 |
| **cclight** | pypi.org/project/cclight | ESP32-C3 + MicroPython（LED 负极接 GND，正极 Pin(3)），USB 连接；pip + Claude Code plugin marketplace 安装 | `cclight state working/idle/input` + daemon |
| **agent-traffic-light-monitor / agent-pulse** | npm / github.com/lzty634158-oss/agent-pulse | 实验性 USB 串口 ESP32-C3 Mini 1，用板载 RGB | 绿=空闲可评审 / 黄=活跃（跑工具或思考）/ 红=阻塞、失败、等权限、需注意；v0.2.2 起工作时黄闪、完成转绿 |
| **icefoxj/claude-monitor** | trendshift.io/repositories/273508 | M5Stack：AtomS3R 立方体（ESP32-S3）显示最紧急状态 + Tab5 平板（ESP32-P4）每会话一格；ESP-IDF 5.5 C++，非 Arduino | 按紧急度取最高 |
| **VibeSignal** | pypi.org/project/vibesignal | USB 状态灯：**Luxafor Flag 2、blink(1) mk2** 等 | hook 装到 Claude Code / Codex（`install-hooks --agent codex`）；无硬件时退化为记录/打印颜色 |
| **Copilot Buddy** | 第三方产品页 | **蓝牙迷你桌宠** | 用灯与动画反映 Copilot CLI 状态（活跃/等待/睡眠）；**实体按钮直接批准或停止助手**，无需切窗口；有庆祝/升级系统与充电电池 |
| **GitHub `status-light` topic** | github.com/topics/status-light | — | 23+ 仓库：Clawd 形象心跳灯（心口随会话发光）、BusyLight（USB/Wi-Fi，ESP32-C6）、CodeLight（BLE 状态灯）、Pi Zero W + WS2812 环、macOS 状态灯、OpenRGB 方案等 |
| **中文教程/开源** | aitntnews / cnblogs FreakEmbedded | 红绿灯模块 + ESP32 开发板；或淘宝现成 ESP32-C3 + 24 颗 WS2812 环（USB-C，免焊，约 20–30 元），10 分钟刷完 | 蓝呼吸=空闲 / 青流动=工作 / 黄慢闪=待审批 / 绿快闪=完成 / 红交替=错误，另加 TTS 与振动 |

### 2.4 编辑器内（VS Code / JetBrains）

| 名称 | 链接 | 它是什么 | 形象机制 |
|---|---|---|---|
| **Claude Pulse**（VSCode-ClaudeCode-Status-Bar） | github.com/zxrrcpandey/VSCode-ClaudeCode-Status-Bar | VS Code 状态栏：working / 任务进度 / 需输入 / 完成，hook 驱动、**无 API 成本** | ⭐ **「Buddy」角色面板随 Claude 状态反应**；展示子代理计数 `⟳ Claude · working 4:46 · 3 agents`；token 用量从 `~/.claude/projects/**/*.jsonl` 本地精确计算；hook 写 `~/.claude/claude-pulse/state/<session_id>.json` 再由扩展 watch |
| **claude-companion-app** | socket.dev/npm/package/claude-companion-app | **VS Code 侧边栏面板 + macOS 菜单栏锚点 + CLI** | ⭐ **动画 mascot 随 Claude 在做什么反应**（idle / thinking / running tools / writing files / errors）+ **工具活动 feed** + 每会话统计（工具调用数、触及文件、错误、时长、token）+ 连续天数。VS Code Marketplace 发布尚在进行中（当前侧载 .vsix） |
| **Pixel Agents** | github.com/pablodelucca/pixel-agents | VS Code 底部面板里的**像素办公室**，观察 `~/.claude/projects/` 的 JSONL，**不需要改动 Claude Code** | ⭐ **One agent, one character**；**6 个不同角色**；打字（写代码）/ 阅读（搜文件）/ 走动（空闲）；等输入或权限时弹**对话气泡**；**Task 子代理作为独立角色并与父角色关联**；办公室布局编辑器（地板 HSB、自动铺墙、50 级撤销、64×64 网格、JSON 导入导出）；音效通知；外部素材目录。VS Code 1.105.0+；MIT |
| **Star UI for Claude Code** | VS Marketplace desanterre.star-ui-claude-code | 活动栏里的 **★ Claude Office 视图**，agent 走到与其活动对应的房间，会绕开墙和家具 | 可选 office / snowy lodge 主题；作用域（workspace vs 全部会话）、空闲子代理显示、**状态栏条目**、停留时长可配 |
| **Agent Arcade** | neura.market / github mshadmanrahman/agent-arcade | VS Code / Cursor 1.85+ 的像素办公室 | 打字/阅读/思考/空闲实时追踪；子代理用虚线连到父角色；**点击角色查看 model、branch、task summary**；完成后「挥手告别」并淡出 |
| **it-crowd-pixel-agents** | socket.dev/npm/package/it-crowd-pixel-agents | Pixel Agents 的叉件，**加 JetBrains 支持（IntelliJ 2025.1+）与独立 CLI server** | 同上 |
| **copilot-agent-pets** | github.com/ifBars/agent-pets | 本地优先桌宠，**镜像 Codex 与 agent 活动**；含 GitHub Copilot CLI hook 包 + MCP server + `/pet` skill | Copilot CLI 模式读 hook 写的**隐私安全状态文件**（可能以 `~/.copilot/session-state/` 修改时间兜底，但不读事件日志内容） |
| **DesktopXPet** | forum.trae.cn/t/topic/41408（Demo） | Windows Electron 桌宠，像素角色 | 5 个状态动画（idle/working/happy/sleeping/error）；通过插件与 **localhost:9527 的 HTTP Push API** 监控 Copilot / Claude Code / Cursor / Ollama；有 IDE 扩展与 Copilot CLI Wrapper；含仪表盘与番茄钟 |
| 官方原生 | anthropics/claude-code issue #99902 | — | 官方 VS Code 扩展的 **"working" 旋转字符 `·✢*✶✻✽` 每 120ms 动画且忽略 reduced-motion** —— 这是一个可访问性投诉，不是功能卖点 |

### 2.5 终端 statusline（用角色 / 形象表达状态的主要战场）

| 名称 | 链接 | 机制 |
|---|---|---|
| **ccusage statusline** | ccusage.com/guide/statusline | 单行：本次会话成本 / 今日 / 5 小时区块成本+剩余时间 / **burn rate（绿 <2000、黄 2000–5000、红 >5000 tokens/min）** / 当前模型 + reasoning effort / 上下文用量。示例 `🤖 Fable 5 (high) \| 💰 $0.23 session / $1.23 today / $0.45 block (2h 45m left) \| 🔥 $0.12/hr \| 🧠 25,000 (12%)`。**用 emoji，无角色** |
| **ccsidekick** | npm（Snyk 描述）| ⭐ **状态栏动画反应角色 + 完整 widget 层**；`npx ccsidekick` 打开设置 UI（首次向导选角色/主题/文案），自动写 statusLine 与 tool-call hook 到 settings.json；`--character spiderman --theme houston`；**零 token 消耗（读磁盘）、本地优先、MIT**；角色作为运行时依赖打包，无下载步骤 |
| **ccpet** | npm | ⭐ **虚拟宠物状态栏**：能量随时间衰减（~0.0231/min），消耗 token 增加（1,000,000 tokens = +1 energy），跨会话持久；**按能量分级动画表情** HAPPY ≥80 `(^_^)` / HUNGRY ≥40 `(o_o)` / SICK ≥10 `(u_u)` / DEAD <10 `(x_x)`；含能量条与会话/上下文统计；`ccpet check --watch` 实时监控（10–300s）；有 graveyard 保留、`leaderboard` |
| **ccfeline / @sebryu/cccat** | npm | ⭐ **猫**：睡觉、呼噜、**走过你的提示符**；`pspsps` 安装（会把 statusLine 钉到该版本）、`shoo` 卸载（还原之前的 statusLine）、`config`、`preview`（5 秒动画预览） |
| **claude-lantern** | github.com/danieledel288-code/claude-lantern | ⭐ **绿灯侠主题**：`/hal`、`/john`、`/guy`、`/lantern-off` **切换角色会改变配色、旋转器、mascot 与 ring HUD 状态栏（计划用量做充能条）**；思考时用盲文点阵**原地重绘约 30 次/秒**的动画构造物（徽章、立方体、锤子、喷气机）。需 Python 3 |
| **clawde** | claudeatlas.com/plugins/adriancisnerosspeed-hub/clawde/ | ⭐ 「住在状态栏、吃上下文窗口的小动画宠物」——上下文低它茁壮，上下文涨满它犯困生病；**可切换皮肤（buddy/cat/slime）**、抓碎屑小游戏、学习你的睡眠时段、每日投喂连续天数 |
| **cc-usage-bar** | npm | `5h 47% Wk 59%` 式订阅用量；支持 Anthropic + 8 家替代供应商（Kimi、GLM/智谱、MiniMax、DeepSeek、StepFun、硅基流动、OpenRouter、Novita）；格式 compact/numeric/time/bar；安装器会保留已有 statusLine |
| **claude-usage-waybar** | github.com/infiniV/claude-usage-waybar | Waybar 模块（Linux/Arch/Hyprland）：读 statusline stdin 的 `rate_limits`（Claude Code v2.1.80+）+ `ccusage daily --json` |
| **clawd-buddy** | pypi | 更像桌面 buddy：Claude Code hook 驱动，庆祝/挥手动画、番茄钟、HTTP webhook、workspace 徽标显示最后发信号的会话 |
| **TokenNotch** | github.com/Keonho-Chu/TokenNotch | ⭐⭐ 刘海 widget，**用官方 Clawd 与 Codex 两个吉祥物**（1:1 sprite 复刻）显示剩余额度 + HP 条 + 重置倒计时；明确声明 Clawd 美术版权与商标归 Anthropic |

### 2.6 AI 伴侣 / 桌宠侧（与 agent 监控交叉，作对照）

| 名称 | 链接 | 备注 |
|---|---|---|
| **super-agent-party** | github.com/heshengtao/super-agent-party | 「自托管 neuro-sama + openclaw」全能 AI 伴侣：自定义 VRM + Live2D 扩展、VTS 联动控制 Live2D 模型与表情、桌面宠物 API、胶囊/手机尺寸窗口；角色卡（SillyTavern）、长期记忆、多角色群聊；任务中心与桌面视觉+键鼠/终端控制；QQ/微信/飞书/钉钉/TG/Discord/Slack bot 一键接；B站/YouTube/Twitch 直播 bot。**约 2,700★ / ~285 fork，2025-03 建、2026-08 最后更新，AGPLv3**（星数为搜索摘要值，未直连复核） |
| **DesktopFriends** | tosuke-sama.github.io/DesktopFriends | 内置 AI Agent 系统的对话型桌宠，支持 OpenAI/Claude/DeepSeek/自定义 API、表情动作控制、内心独白、日程管理、人设提示词 |
| **Pets / Antigravity-Pets** | github.com/Avneesh-Pratihast/Pets | 面向开发者的 AI 桌面伴侣 v2.5.3：**用 SMTC 做音乐检测**（Chrome/Brave/Edge/Spotify/VLC/YouTube Music 兜底轮询）；明确的 **Dance vs Focus 切换**，跳舞时显示均衡器条与音符；番茄钟、物理投喂、屏幕边缘攀爬、编码实时反应；Windows 安装包/免安装 |
| **Mini-tanjiro** | npm | 透明点击穿透覆盖层，坐在所有窗口上方；**似乎不对前台应用有反应** |
| **Neko – Desktop Pet** | itch.io | 用户反馈 **Spotify 不注册**（歌名检测/行为失效）；1.2 加了「不管有没有音乐都跳舞」选项并改进音乐检测 |
| **Desktop Character Layout** | Steam 4178760 | 分屏工作区/角色桌面，角色始终可见，可分区背景；**不随运行的应用变化** |
| **天选姬（ASUS TX Mascot）** | — | 属已知基线（安装包内含 `ForegroundProcess.dll`，[推测] 对前台进程有反应）。**本次未复核**，列出仅供交叉参考 |

---

## 3. (b) 详细list：付费皮肤 / 形象变现生态（判断商业模式可行性）

| 产品 / 生态 | 形态 | 价格与数据 | 日期 |
|---|---|---|---|
| **Desktop Mate**（infiniteloop，Steam 3301060） | 免费本体 + 付费角色 DLC（平台化） | 单角色 DLC **2,200 日元**（促销 20% OFF = 1,760）；角色含初音未来、雪未来 SNOW MIKU 2025、ずんだもん、小春六花、重音テト、巡音ルカ、2B、超绝最可爱天使酱、あめちゃん、花隈千冬、KUROMI、博丽灵梦、キズナアイ；KUROMI DLC 美区 8.19 美元、台区 NT$328、博丽灵梦 76 元。**累计下载：24 小时 13 万 → 2 周 100 万 → 2025-03-17 130 万 → 2025-07 150 万（海外约 6 成）→ 2025-11-20 170 万 → 2026-03 190 万**。2026 春季促销全 11 款 DLC 20% OFF | 2025-01-08 上线；最新 2026-08 キズナアイ DLC |
| **Sia Desktop Pet**（Steam 5035420） | 本体 + 亲密度解锁商店 + 衣柜 | **11 套服装**，季节限定服装只在当季出售；「病娇模式」在商店解锁，每项能力单独购买、单独开关。官方披露部分美术与商店图由 AI 生成后人工编辑 | 2026-11 发行 |
| **桌面精灵**（Steam 3003300） | 换装 / 染色 + 创意工坊 | 商店卖「**豪华版：本体 + 全部服装 DLC**」，4 件套省 10%（约 S$10.42）；工坊可改角色模型、服装贴图、对话/语音/动画、AI Chat | — |
| **AI桌面宠物**（Steam 4227700） | 内置本地大模型 + VRM/Spine/静态图 + 创意工坊 | 台区 NT$275（-20% 后 NT$220）；日区 ¥1700（-20% 后 ¥1360） | SteamDB 有补丁历史 |
| **猫娘计划 Project N.E.K.O.** | 「AI 原生桌面伴侣平台」 | 买本体得核心 AI 伴侣体验；**未来计划联合第三方推付费 DLC 与独立游戏**（小游戏、桌游、视觉小说、MMO 元宇宙）；工坊可下角色、性格模板、语音包、服装、Live2D/VRM 模型，**记忆跨体验同步** | 2026-01-31 发布 |
| **搜狗（腾讯）输入法宠物皮肤** | 输入法皮肤 / 电子宠物 | 单款皮肤售价 **8 元，已下载 4.8 万次**；官方商城创作者分成 **付费金额的 30%–50%**；普通皮肤若 1 万次下载、单价 2 元，创作者收入约 6,000–10,000 元；爆款 IP 联名单款可达 10 万元（不适用个人创作者）；小红书/闲鱼定制皮肤客单价 1–5 元。**动态皮肤/桌宠类产品年收入约 3–5 亿元（Statista 2022，经中文二手转述，未验证原始报告）**；动态皮肤用户付费率年均增长 12%，桌宠类皮肤占总皮肤下载量 15%。玩法：与暹罗厘普联动做拓麻歌子式宠物（喂饭/玩耍/洗澡，**左上角爱心代表宠物状态，不同心级不同形象**）、心情日记皮肤可手动切心情、奇迹多栋支持换装、小龙格林 2.0 用「血槽」显示打字量并逐级解锁功能、Loopy/卡皮巴拉联名有**「打字跟随」——IP 形象模仿用户打字状态，速度一致、敲键位置 1:1 复刻**。搜狗官网提供皮肤编辑器形成 UGC | 报道 2024–2026 |
| **Razer AVA / Project AVA** | 硬件全息 AI 伴侣（CES 2026） | 5.5 吋 3D 全息柱，可换 avatar（Ava 能量团 / **Kira 猫娘（默认）** / Zane 男性 / **Faker 电竞选手** / Sao 偶像），实时眼动、表情、口型同步（与 Animation Inc. 合作）；性格可配（大胆/调皮/冷静/友善）；**LLM 无关**（可选 Grok 或 ChatGPT）。美区 **$20 可退押金预订**，整机价格未公布，预计 2026 下半年发货。Razer 官网有「Avatar Customization」区块，**公开招募游戏开发商把自家角色带进 AVA**。⚠️ **「付费皮肤 / Faker×T1 联名作为持续收入 / Razer AIKit / 开源全息 SDK」这些说法只出现在非官方站点 avahologram.com，Razer 官方未确认** | CES 2026（2026-01） |
| **Petdex**（crafter-station/petdex） | 开源 agent 宠物画廊 | **4,667–4,675+ 个免费动画宠物**，覆盖 Codex / Claude Code / DeepSeek Harness / Hermes / OpenCode / Gemini CLI 等；每个宠物 = `pet.json`（含 idle/working/sleeping 等 **9 个动画状态**与帧时长）+ `spritesheet.webp`；`npx petdex install boba` 一键装，`npx petdex submit ./my-pet/` 提交；有桌面 App 让宠物实时对 agent 活动反应。**官方 About/FAQ 明确说 Petdex 不赚钱：无广告、无附加销售（no add-ons/upsells）、无遥测（只统计哪些宠物被安装）**；创作者有 `@Creator`/`@Collector`/`@Featured Creator` 角色；社区规则明确**禁止 DM 推销与 NFT shilling**；IP 归原作者并要求署名 | 2026 |
| **Eilik**（Energize Lab，深圳） | 实体桌面陪伴机器人 | 1.54" OLED 当脸，**4 种基础情绪（normal/happy/angry/sad）**、**1,000+ 表情**、4 舵机、3 个触摸传感器 + 振动/红外/麦克风；头/腹/背触摸触发不同情绪，怕高（被举起会焦虑）；**完全离线、无 WiFi/App/账号**；约 **$139.99**（次代 Eiliko 起 $59.90）；**不是 LLM 对话式**，可选 AI Station 配件加 ChatGPT。多台可互相识别、聊天、玩耍、吵架。全球 60,000+ 用户 | — |
| **Ropet**（北京梦游智能） | 实体毛绒 AI 宠物 | 圆形毛绒 + 前面屏幕大眼，会眨眼、眼神接触、显示情绪；识别人脸并记住主人，通过玩耍成长出个性；支持音乐/动作/语言/表情识别、仿生毛、模拟体温、饥饿反应；**本机处理数据**，附赠物理摄像头遮盖眼罩；约 **$199–249**，Kickstarter 2024-04（~1,759 支持者 / $214K），CES 2025 展出，2025 双十一国内开卖 | 2025 |
| **Anki Vector 2.0** | 实体桌面机器人 | 人脸识别 + ChatGPT 语音；**有订阅坑**；交互以机械音/音频为主 | — |

---

## 4. 对「最独特卖点」的正面判断

### 4.1 站得住的判断

1. **「原创角色 + 感知本机 AI agent 状态并做出行为变化」这一层——已经拥挤，且第一方已进场。**
   OpenAI Codex Pets（官方、2026-05）、Anthropic Claude Code Buddy + Clawd（官方）、GitHub Copilot CLI mascot（官方）、微软 Mico、Meta Muse/Jolly、OpenAI Dots，加上 30+ 个开源/第三方桌宠与状态指示器。**「有个角色反映 agent 状态」在 2026 年 10 月已经不是差异化，而是基本盘。**

2. **「同一个角色在不同 agent 工具下呈现不同面貌」这条轴——确实空白。**
   我系统检索了中/英/日文近 20 组关键词（§7），**没有找到任何产品把它作为功能或卖点**。现有实现全部落在另外两条轴上：**同一角色随状态变**、**不同工具配不同角色**。

3. **但这条轴的机制本身不构成壁垒。**
   外设四巨头（Razer / Logitech / Corsair / Elgato）十年来的 app-aware profile 已经证明这个机制是标准件；Apple 已经把「按 App 触发、自动换整套外观」推给十亿用户。**所以这条轴能作为「体验设计上的差异化触点」，不能作为「技术护城河」来主张。**

### 4.2 必须正面回应的反向证据（重要，别自我说服）

- **Hopet 明确反其道**：多会话**聚合到一只宠物**、按优先级显示最高状态，只「高亮是谁在找我」。它的设计前提是「一个用户只需要一个形象」。
- **VibeMon 2.0 从「每项目一个窗口/角色」收敛到「单一角色 + 跟随气泡」**，blog 标题直接是 "One Character, Smarter Than Ever"。同样是减认知负担。
- **LLMPET 的 dsh 皮肤是「另一个 agent 换另一个角色（鲸鱼女仆）」，而不是「同一角色换面貌」**——市场用脚投票选的是「一眼分清是谁」，而不是「同一个角色换皮」。
- **这些产品都在减少「多个形象」而不是增加「同一形象的多变体」。**

→ **结论：「形象差异化」的正确论证路径不是「机制新」，而是「用户是否真的在不同 agent 之间需要区分认知」。需要用户验证，不能用「没人做过」当成需求存在的证据。** 反过来说，如果差异化点的真实价值是「一眼知道现在是谁在跑」，那 OC-Claw / VibeMon 的「换角色」路线已经被验证更容易被理解。

### 4.3 额外观察（可能比「形象差异化」更值钱的空白）

1. **AI agent 桌宠没有出现付费皮肤生态。** Petdex 是全免费且官方明确不商业化（无广告、无 upsell）。付费皮肤只存在于更泛的桌宠（Desktop Mate 2,200 日元 / Sia 11 套服装 / 桌面精灵豪华版）与**中文输入法皮肤生态**（8 元单款 4.8 万下载、创作者分成 30–50%、品类年收入 3–5 亿元量级）。**「原创形象 + 付费皮肤」的组合在 agent 桌宠里还没人做。**
2. **「原创形象」本身是稀缺的。** 现有产品大量借用 Claude 官方 Clawd（`clawd-on-desk`、`clawd-conduit`、`TokenNotch`、`clawd-mochi`）、Codex 吉祥物、或 Petdex 的公共素材。**自研原创形象 + 品牌化，是避开同质化最直接的路**，但也意味着要独立承担美术与 IP 成本。
3. **「陪伴感 + 工作状态」双核心的第二个核心（陪伴）在 agent 桌宠里普遍很薄。** 大多数只做「状态映射 + 审批」；只有 AgentPet（进化/成就/排行榜）、ClaudePet（季节皮肤）、ccpet（能量衰减/graveyard/leaderboard）、clawde（小游戏/投喂连续天数）、super-agent-party（角色卡/长期记忆/多角色群聊）在认真做养成与陪伴。
4. **「硬件延伸」已被验证为可行的第二条产品线**：vibe-lamp / cclight / agent-pulse / vibe-pet（BLE 到 Wio Terminal、ESP32-S3）/ Copilot Buddy / VibeSignal。对自有形象来说，硬件是把「形象」实体化的天然变现路径。

---

## 5. 「未找到」清单（明确找不到的，不代表不存在）

1. **❌ 未找到任何产品以「同一个角色 / 形象随所运行的 AI agent 工具自动呈现不同面貌（换装 / 换配色 / 换表情 / 换行为）」为功能或卖点。** 最接近的三个是：
   - OC-Claw「pair different agents with different characters」（**换角色**，非同一角色变体）
   - VibeMon「按工具自动映射角色」+ Character Lock（同上）
   - dsh-lived-pet「装扮自动穿脱：读文件戴眼镜、写代码贴贴纸」（**换装，但触发源是 agent 状态而非 agent 身份**）
2. **❌ 未找到 AI 编码 agent 桌宠的付费皮肤 / DLC 商店。** Petdex 明确声明非商业化、全部免费；Codex Pets 的自定义宠物由用户自制并用 `/hatch` 生成（消耗 Codex 额度但不售卖）；clawd-on-desk 支持导入 Codex Pet zip，未见商店。
3. **❌ 未找到 Claude Code / Codex / Copilot 之外，任何 agent 厂商的官方吉祥物。** 官方 mascot 目前只有 Anthropic Clawd/Buddy、OpenAI Codex Pets、GitHub Copilot CLI mascot、微软 Mico。**注意：搜索结果中完全没有提到 Google Gemini 的吉祥物或桌宠**（多篇 2026-10 的「AI 巨头吉祥物」报道只覆盖 OpenAI / Meta / Anthropic / 腾讯）。
4. **❌ 未找到 VTuber 生态中「检测当前游戏 → 自动换装」的开箱即用插件。** 只有可编程的构建块：VTube Studio API（插件可激活 hotkey、注入自定义参数，「literally anything to control your model」）、ViewLink（Live2D 角色对 VR 游戏画面反应）、Warudo 的 Material Changer（可接任意 Blueprint 触发器）、Dalamud 的 DynamicBridge（FFXIV 内按条件预设自动换外观）。**「没有现成的 game-detection → outfit-swap」是明确的空白。**
5. **❌ 未找到「用角色形象表达 token 用量 / 额度 / 花费」的产品。** 用量类工具（ccusage、SlopBar、QuotaBubble、ai-taskbar-widget…）全部用圆环 / 数字 / emoji。最接近的是 **TokenNotch**（用官方 Clawd sprite + HP 条表示剩余额度）和 **ccpet**（用宠物存活状态表示 token 消耗）。
6. **❌ 未找到「桌宠按检测到的前台应用切换皮肤」的商业化产品。** 找到的都是：按应用调整**行为**（MateEngine 白名单跳舞、NekoAI 活动窗口、Shimeji titles.conf）或按应用**暂停**（Lively / iWall）。**专利里明确描述了「按运行的应用更新角色的姿势/显示应用类型标识」（US20240176626）、「读软件运行状态实时更新 3D 虚拟角色形象」（CN114296853A）——即技术上被主张过，但未见产品落地。**
7. **❌ 未找到 Claude Code / Codex 官方文档中关于「形象随 agent 变化」的任何描述。**

---

## 6. 不确定项 / 需复核（不要当成事实用）

| # | 不确定的说法 | 状态 |
|---|---|---|
| 1 | **Claude Code Buddy 现在是否还存在** | **未证实**。有 GitHub issue 称前端在 **v2.1.97 被移除**；另有来源说命令在 4/1–4/7 孵化窗口后仍可用。单一来源，未复核 |
| 2 | **Buddy 的 `buddy_react` 端点数据上报** | **未证实**。issue #67929 称孵化时客户端读 `package.json`（name/description）与 `git log`（最近 3 次提交），后续回合/错误/大 diff 时把 ≤5,000 字符的 transcript 片段发到 `/api/organizations/{uuid}/claude_code/buddy_react`，且开启 UI 未披露。**是用户提出的疑问，非 Anthropic 声明** |
| 3 | **Codex Pets 在 EEA / 英国 / 瑞士被屏蔽** | **单一来源**（unwire.hk），OpenAI 未给原因与时间表 |
| 4 | **Pixel Agents 星数** | 不同来源差异巨大：约 2,800（早期 addROM）/ ~6,400（Zenn，2026-04）/ 8,635（Snyk）/ 9,281（Zenn 实测 2026-09-14）/ 9,430（FOSSHUNTER）。**取 ~8.6k–9.4k（2026-09/10）并视为近似**。仓库归属也有 `pablodelucca/pixel-agents`、`pixel-agents-hq/pixel-agents`、`hootbu/pixel-agents` 多个 |
| 5 | **clawd-on-desk 约 2.6k★** | 来自腾讯云开发者文章（2026），**未直连复核**；另属另一路基线 |
| 6 | **super-agent-party 约 2,700★ / ~285 fork** | 来自搜索摘要（relatedrepos / skillsllm），**未直连复核**；另注意仓库同时存在 `heshengtao/` 与 `Ran-Chou/` 两个路径 |
| 7 | **clawd-on-desk 的三套主题是否按 agent 工具自动切换** | **未证实**。搜索摘要只说「有三套主题 + 可导入 Codex Pet zip」，未说明切换是否自动 |
| 8 | **Razer AVA 的付费皮肤 / Razer AIKit / 开源全息 SDK** | **未证实**，只出现在非官方站点 avahologram.com。Razer 官方只确认了 $20 可退押金、可换 5 个 avatar、招募第三方角色、LLM 无关 |
| 9 | **「动态皮肤/桌宠类产品年收入约 3–5 亿元」等中文市场数据** | 来自中文二手转述，标注为 **Statista 2022**，**未验证原始报告**。数字层级看起来可疑，**不要直接引用** |
| 10 | **Copilot Buddy（蓝牙物理桌宠）** | 只在一份工具目录站（SheepNav）出现，**未找到厂商官网或发布方**，未证实 |
| 11 | **天选姬 `ForegroundProcess.dll` 的作用** | 属已知基线推测，**本次未复核** |
| 12 | **本次调研无法直接打开任何原始页面** | 见 §0。所有 GitHub star、changelog 版本号、issue 编号均来自搜索引擎返回的页面摘要。**若要把某条写进产品决策，建议在有正常网络的环境下逐条复核原始页面** |
| 13 | **Claude Code `AskUserQuestion` 状态** | job-finish 的描述里提到它作为独立状态；未在官方文档复核 |
| 14 | **`vibe-monitor` vs `vibemon`** | 同一项目的旧/新 npm 包名；2.0 blog 的角色表列 vibemon/clawd/kiro/claw/clawd，而 npm README 还列了 navy 的 **codex**。**两处列表不完全一致** |

---

## 7. 检索关键词清单（供复核与后续补漏）

**英文**：Claude Code status bar / menu bar app indicator · AI agent notifier menu bar tray 2025 · AI coding agent status indicator · Claude Code statusline mascot animated · ccusage token usage menu bar · desktop pet reacts to Spotify now playing · desktop pet skin DLC detect running application · desktop mascot reacts to foreground window app-aware · Shimeji app-aware · Razer Chroma app-aware lighting profiles · Logitech G HUB per-application profile automatic game detection · SteelSeries Corsair iCUE per-application lighting · Elgato Stream Deck app-aware profiles · app-aware avatar wallpaper running application · VTuber avatar outfit game detection · Slack status auto based on active app · Apple Focus mode auto change watch face · OpenAI Codex Pets · Claude Code Clawd mascot · Anthropic Claude Code Buddy virtual pet · GitHub Copilot mascot status · Microsoft Copilot Mico · desktop companion robot expressive face Eilik Ropet · coding agent pet skins marketplace paid · Petdex

**中文**：桌宠 皮肤 检测应用 切换形象 状态 · AI agent 状态 桌面悬浮窗 形象 角色 监控 · AI 编程 状态指示 桌宠 · Windows 托盘任务栏 Claude Code Codex 状态提醒 · 输入法 宠物 皮肤 付费 下载量 · AI 巨头 吉祥物 卖萌 · 桌宠 AI agent 皮肤 商店 付费 · 同一个角色 不同 AI agent 不同皮肤 换装 · Desktop Mate 桌宠 DLC 角色 收费 销量

**日文**：Desktop Mate DLC キャラクター 価格 · デスクトップマスコット アプリ連動

---

## 8. 附：全部条目汇总表

见 §2（a 类详细表）与 §3（b 类付费生态表）。核心条目速查：

| 名称 | 链接 | 类型 | 它是什么 | 状态表达方式 | 形象机制 | 备注 |
|---|---|---|---|---|---|---|
| **OpenAI Codex Pets** | developers.openai.com/codex/app/settings#codex-pets | 官方桌宠（Codex 内置） | 浮在桌面的 agent 覆盖层，可点宠物回消息 | 气泡 + 动画：Running / Needs input / Ready / Blocked | 8 个内置 + `/hatch` 自定义（`$skill-installer hatch-pet`） | **2026-05 上线 Win/mac**；EEA/UK/CH 未开（单一来源）；自定义宠物耗 Codex 额度 |
| **Anthropic Claude Code Buddy** | `/buddy`（v2.1.89） | 官方终端宠物 | 输入框旁 ASCII 精灵，随编码动画 | 动作反应 | 18 物种 / 5 稀有度 / 1% Shiny / 6 眼型 / 8 帽子 / 5 属性；由 UUID 确定性生成 | 源自 2026-04-01 愚人节彩蛋（盐值 `friend-2026-401`）；v2.1.97 移除说法未证实 |
| **Clawd（Anthropic 官方 mascot）** | anthropics/claude-code issue #8536 | 官方吉祥物 | 像素橘色螃蟹，CLI 内 sprite，**4 姿势**，色值 rgb(215,119,87) | 启动画面问候 | 单一固定形象 | 官方文档长期未收录；周边在开发者活动发放、网店约 $150 玩偶 |
| **GitHub Copilot CLI mascot** | github/copilot-cli changelog v1.0.15 | 官方 mascot | 交互模式内置 | **眨眼动画** | 单一形象 | 2026-04-01 changelog |
| **Microsoft Mico** | Copilot Voice | 官方 AI 角色 | 团状角色出现在语音模式 | 移动/说话/笑/跳舞/变形/**变色** | 可换外观与声音；Learn Live 变戴眼镜导师 | 2025-10-23 发布；美/加/英（一处说仅美）；40+ 语言 |
| **claude-island** | github.com/farouqaldori/claude-island | macOS 刘海 | 灵动岛式通知 | 刘海展开动画 | 无 | ✅ 就地审批 + 聊天历史；macOS 15.6+ |
| **claude-status-bar** | github.com/m1ckc3s/claude-status-bar | macOS 菜单栏 | 极小 App | 动画图标 + 黄点 + 计时 | **图标动画可选 Spark/Unicode/Clawd/Orbit** | `brew install --cask` |
| **Agent Signals** | producthunt.com/products/agent-signals | macOS 菜单栏 | 红绿灯 | 🔴🟡🟢 | 无 | 仅 iTerm2；MIT |
| **CC Lights** | global.v2ex.co/t/1226660 | macOS 菜单栏 | 红绿灯 + 跳转终端 | 颜色 | 无 | Terminal/iTerm2/Ghostty/cmux |
| **Agent Signal Bar** | github.com/guan-ops/Agent-Signal-Bar | macOS 菜单栏 + 悬浮 | 信号灯 + 用量 | 红黄绿 + 优先权 | 无（有主题） | 自动检测 Codex；Claude 走 hook |
| **AgentTray** | github.com/sprklai/agenttray | Tauri 托盘 | 跨平台 | 彩色托盘点 | 无 | Linux 主平台 |
| **AI CLI Complete Notify** | github.com/ZekerTop/ai-cli-complete-notify | Tauri 托盘 | 完成提醒 | 多通道通知 | 无 | v2.17.0 |
| **Notchcode** | github.com/billxby/notchcode | 顶部状态条 | Win+Mac | CC/CD 徽标 | 无 | 2026-06 加 Win |
| **AI Light** | github.com/LeoKemp223/ai-light | 桌面悬浮红绿灯 | Tauri | 四态颜色 | 无 | 按 git 根目录聚合 |
| **QuotaBubble** | pypi.org/project/quotabubble | 悬浮气泡 | 用量 | 气泡 + 倒计时 | 无 | Win 为主 |
| **vibe-lamp** | github.com/laofahai/vibe-lamp | 硬件（ESP32-C3） | 物理状态灯 | 颜色 + 动画 | 无 | 状态优先级明确 |
| **cclight** | pypi.org/project/cclight | 硬件（ESP32-C3） | LED 指示 | 颜色 | 无 | |
| **icefoxj/claude-monitor** | trendshift.io/repositories/273508 | 硬件（M5Stack） | 桌面状态灯 | 立方体 + 平板 | 无 | |
| **VibeSignal** | pypi.org/project/vibesignal | 硬件（Luxafor/blink(1)） | USB 灯 | 颜色 | 无 | |
| **Copilot Buddy** | SheepNav 目录 | 硬件（蓝牙） | 物理桌宠 | 灯 + 动画 | — | **未证实**，无厂商页 |
| **Claude Pulse** | github.com/zxrrcpandey/VSCode-ClaudeCode-Status-Bar | VS Code 状态栏 | 状态 + 子代理 + token | 状态文字 + 计时 | ⭐ **Buddy 角色面板随状态反应** | hook 驱动零 API 成本 |
| **claude-companion-app** | socket.dev/npm/package/claude-companion-app | VS Code 侧边栏 + 菜单栏 + CLI | 侧边栏面板 | 工具活动 feed + 每会话统计 | ⭐ **动画 mascot 随状态反应** | Marketplace 发布进行中 |
| **Pixel Agents** | github.com/pablodelucca/pixel-agents | VS Code 像素办公室 | 观察 JSONL | 打字/阅读/走动/气泡 | ⭐ **一 agent 一角色，6 个角色** | MIT；~8.6k–9.4k★（近似）；子代理独立角色 |
| **Star UI for Claude Code** | VS Marketplace desanterre.star-ui-claude-code | VS Code 活动栏 | Office 视图 | 角色走到对应房间 | ⭐ 同一角色按活动换房间 | 可选主题 |
| **Agent Arcade** | neura.market / github mshadmanrahman | VS Code/Cursor | 像素办公室 | 角色动画 | ⭐ 点击看 **task summary** | 子代理虚线连接 |
| **ccsidekick** | npm | 终端 statusline | 角色 + 成本/用量 widget | 角色反应 | ⭐ **可反应角色 + 主题**（`--character spiderman`） | MIT；零 token；设置向导 |
| **ccpet** | npm | 终端 statusline | 虚拟宠物 | **颜文字分层表情** `(^_^)/(o_o)/(u_u)/(x_x)` | ⭐ 能量随 token 衰减/增长 | 有 leaderboard、graveyard |
| **ccfeline / @sebryu/cccat** | npm | 终端 statusline | 猫 | 睡觉/呼噜/走过提示符 | ⭐ 单一猫 | `pspsps`/`shoo` |
| **claude-lantern** | github.com/danieledel288-code/claude-lantern | 终端 statusline | 绿灯侠主题 | 盲文点阵动画 30fps | ⭐ **切换角色改变配色/旋转器/mascot** | 需 Python 3 |
| **clawde** | claudeatlas.com/plugins/.../clawde/ | 终端 statusline | 宠物 | 上下文满则困/病 | ⭐ **皮肤 buddy/cat/slime** + 小游戏 | MIT |
| **TokenNotch** | github.com/Keonho-Chu/TokenNotch | MacBook 刘海 | 额度 widget | HP 条 + 重置倒计时 | ⭐⭐ **官方 Clawd 与 Codex 两个 mascot** | Clawd IP 属 Anthropic |
| **ccusage statusline** | ccusage.com/guide/statusline | 终端 statusline | 成本/用量 | emoji + 文本 + burn rate 变色 | 无角色 | v16.1.2 |
| **SlopBar** | github.com/thelioo/slopbar | Windows 任务栏 | 额度圆环 | 圆环 + 数值 | 无 | 额度用尽自动切账号 |
| **ai-taskbar-widget** | github.com/KimJinWooDa/ai-taskbar-widget | Windows 任务栏 | 额度 | 颜色 + 徽标 | 无 | 韩语 |
| **Gengchou** | winget ynjmxn.Gengchou | Windows 任务栏/托盘 | 额度 | 数值 | 无 | v2.4.0 |
| （macOS 菜单栏 CLI 状态监控） | App Store id6778941425 | macOS 菜单栏 | 只读 transcript 推导状态 | 通知 + 角标 + 检查器 | 无 | 免费 + 内购 |
| **MateEngine** | store.steampowered.com/app/3625270 | 3D 桌宠（VRM） | 桌面伴侣 | 跳舞/待机 | ⭐ **音频源白名单（Spotify/Chrome）→ 跳舞** | 2025-04-16；Steam $3.99，GitHub 免费版 |
| **MMDEngine** | Steam 2241940 | 桌宠 | 任务栏 | 随音乐跳 | ⭐ 实时检测本地/在线/游戏 BGM | Win 7–11 |
| **HearHen** | Steam 4341080 | 桌宠 | 免费 | 随任意 PC 音频反应 | ⭐ 全系统音频 | 2026-02；Win 10 64bit |
| **NekoAI** | naudycastellanos.com/en/projects/nekoai | 桌宠（Tauri+Rust） | 个人项目 | 在窗口间游荡 | ⭐ **Win32 活动窗口 + 空闲时间**；>90 分钟提醒 | |
| **Shimeji（含 Linux 叉件）** | github.com/Pro-Prietary/sayori-shimeji-linux | 桌宠 | — | 攀爬窗口/掉落 | ⭐ **`titles.conf` 窗口标题白名单**；窗口层级检测 | 中文版称会学习窗口管理习惯 |
| **dsh-lived-pet** | github.com/deanzhang2026-max/dsh-lived-pet | Live2D 桌宠 | DSH agent 状态 | 状态动画 | ⭐⭐ **换装系统：4 槽位 / 20 素材 / 读文件戴眼镜、写代码贴贴纸**；不覆盖手动槽位 | 触发源是**状态**非工具身份 |
| **whale-girl-plus** | dsh.deepseek404.com | DSH 插件 | agent 状态 | 动作 | ⭐ 多服饰 + **抽到打网球动作自动换运动服** | |
| **ChatWaifu** | store.steampowered.com/app/2331610 | Steam 桌宠 | AI 伴侣 | — | 服饰/姿势**「auto switched by AI」** | 措辞含糊，未证实 |
| **OC-Claw** | github.com/rainnoon/oc-claw | 桌宠 | 刘海/任务栏 | 工作动画/空闲休眠 | ⭐ **"pair different agents with different characters"** | v1.8.6；开源免费；winget `rainnoon.OC-Claw` |
| **VibeMon**（vibe-monitor） | npm vibemon / nalbam.github.io | 桌宠 | 像素角色 | 状态动画 | ⭐ **按工具自动映射角色**（clawd/codex/kiro/claw/vibemon）+ **Character Lock** | 2.0 收敛为单角色 + 跟随气泡；可输出到 ESP32 显示 |
| **Vibe Pet**（Seeed） | github.com/Seeed-Solution/vibe-pet | 桌宠 + 硬件 | 多 agent 监控 | 状态动画 | ⭐ **每 agent 独立 pet card** + **Petdex 4,600+ 免费宠物**或一键自建 | 兼容 15+ agent；BLE 到 Wio Terminal/ESP32-S3 |
| **Petdex** | github.com/crafter-station/petdex · petdex.dev | 宠物画廊 | 公共素材库 | 9 个动画状态 | 4,667+ 免费宠物；`npx petdex install` | **官方明确不商业化（无广告/无 upsell）** |
| **Hopet** | github.com/BinaryFroggy/Hopet | 桌宠（macOS） | 像素海豹 | 8 态状态机 | **多会话聚合到一只宠物**，按优先级取最高 + 高亮「谁在找我」 | 反向设计证据；权限可原位 Allow/Deny |
| **LLMPET** | github.com/myunwang/LLMPET | 桌宠 | 多 agent 工作台 | 表情 + 气泡 | ⭐ **dsh 有独立「鲸鱼女仆」皮肤，23 动画 / 21 状态** | MIT；一键审批；跨 agent 交接 |
| **ClaudePet** | devthekiwi.github.io/ClaudePet | 桌宠（Dock） | Claude 专用 | 气泡 | ⭐ **季节皮肤（右键手动切）**；多会话不同颜色宠物 | macOS 13+/Win 10+ |
| **AgentPet** | github.com/ntd4996/agentpet | 桌宠 | tamagotchi 养成 | 进化阶段 | ⭐ **按项目分配宠物**；5 阶段进化 + 14 成就 + 排行榜 | 基线已列，此处仅作对照 |
| **super-agent-party** | github.com/heshengtao/super-agent-party | AI 伴侣平台 | VRM/Live2D | 表情/动作 | 角色卡、长期记忆、多角色群聊、VTS 联动 | ~2,700★（未复核）；AGPLv3 |
| **Desktop Mate** | store.steampowered.com/app/3301060 | Steam 桌宠平台 | 免费本体 | 动作/语音 | ⭐ **付费角色 DLC 2,200 日元** | 190 万下载（2026-03） |
| **Sia Desktop Pet** | store.steampowered.com/app/5035420 | Steam 桌宠 | 亲密度解锁 | — | ⭐ **11 套服装 + 季节限定 + 商店逐项购买** | 2026-11 发行 |
| **桌面精灵** | store.steampowered.com/app/3003300 | Steam 桌宠 | 换装/染色 | — | 豪华版本体 + 全部服装 DLC（省 10%） | 创意工坊 |
| **猫娘计划 Project N.E.K.O.** | ku.gamersky.com/2026/project-n-e-k-o | AI 桌面伴侣平台 | — | — | **计划第三方付费 DLC**；工坊角色/性格/语音/服装 | 2026-01-31 |
| **Razer AVA** | razer.com/de-de/razer-ava | 硬件全息 AI 伴侣 | CES 2026 | 眼动/表情/口型 | ⭐ **5 个可换 avatar + 招募第三方角色** | $20 可退押金；付费皮肤说法**未证实** |
| **Eilik** | — | 实体机器人 | 桌面 | 4 情绪 / 1,000+ 表情 | 单一形象；触摸驱动 | ~$139.99；完全离线 |
| **Ropet** | — | 实体毛绒机器人 | 桌面 | 屏幕眼 + 情绪 | 单一形象；人脸识别、个性成长 | ~$199–249 |
| **搜狗输入法宠物皮肤** | socialbeta.com/article/109718 | 输入法皮肤 | 7.83 亿输入法用户 | **爱心心级 = 宠物状态**；血槽 = 打字量 | ⭐ **付费换装 / 打字跟随 / 心情切换 / UGC 编辑器** | 单款 8 元 4.8 万下载；分成 30–50% |
| **Apple Focus → 表盘/锁屏/主屏** | support.apple.com（Use Focus on Apple Watch） | OS 级 | 十亿级装机 | 表盘/锁屏/主屏整套切换 | ⭐⭐ **可按「使用某 App 时」触发 + Smart Activation** | watchOS 8 起，iOS 16 起含锁屏主屏 |
| **Razer Synapse / Chroma** | mysupport.razer.com a_id/1940 | 硬件外设 | — | 灯光 | ⭐ **profile 绑 .exe，焦点切换**；Chroma Connect；**Screen Sync 分析前台窗口画面** | Synapse 3 自 2026-02-03 停更 |
| **Logitech G HUB / LGS** | support.logi.com 6505616660759 | 硬件外设 | — | DPI/灯光/宏切换 | ⭐ **自动游戏检测（默认关闭）+ 进程名绑定** | 抢前台会回落默认 |
| **Corsair iCUE** | help.corsair.com 4424235581837 | 硬件外设 | — | 灯光 | ⭐ **Linked Applications**；Game SDK + Application/Device Control | iCUE 3 用 "Link profile to program" |
| **Elgato Stream Deck** | elgato.com Smart Profiles | 硬件外设 | — | 按键布局切换 | ⭐ **Smart Profiles：绑 Application** | 不支持 UWP；软件须最小化 |
| **Keylegend** | github.com/Eistee82/Keylegend | 第三方 | Razer Chroma 键盘灯 | 按键颜色含义 | ⭐ **约 90 个应用 profile** | 应用 profile 优先于通用游戏 profile |
| **Lively Wallpaper / iWall** | github.com/lively-community/lively | 壁纸引擎 | — | 暂停/恢复 | ⭐ **前台进程 / 全屏 / D3D 检测** | 性能向 |
| **SynClub** | appbank.net/2026/09/12 | AI 聊天 App | — | 20+ 心情立绘 + 场景背景 | ⭐ **心情变化 → 立绘切换** | 2026-09-12 |
| **OpenAI Dots** | — | 官方 AI agent | — | — | ⭐ **多个可自定义角色**（Alfred/Felipe/Dottie） | 2026-09-29 发布 |
| **Meta Muse / Jolly + Muse Charm** | — | 官方 AI agent | — | — | 单一毛绒角色 + 电子宠物钥匙扣硬件 | 2025-09-08 |
