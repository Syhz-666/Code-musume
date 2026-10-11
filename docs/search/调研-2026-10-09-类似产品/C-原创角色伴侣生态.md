# C 路调研：原创角色桌面伴侣生态（2026-10-09 现状）

> 调研时间：2026-10-09。调研方式：GitHub API（可直连，数据可信）+ 联网搜索摘要（不可直连 Steam / 官网 / 多数博客，凡来自搜索摘要的数据均标注）。
> 本机网络限制说明：本次会话中 WebFetch 全域名不可用，Bash 只能连通 api.github.com / github.com / 少数白名单域名；Steam 商店页、SteamDB、Reddit、官网等**无法一手打开**，相关数据来自搜索引擎返回的页面摘要，已在文中逐条标注来源与日期，可疑处标「未证实」。
> 所有 GitHub star / 提交时间均为 2026-10-09 通过 GitHub API 抓取的原始值。

---

## 0. 一句话结论

2026 年 10 月的桌宠生态已经分裂成两条清晰的路线：
**(A) 角色平台路线**——以形象素材（Live2D / VRM / 精灵图）为中心，卖角色、卖皮肤、卖工坊内容（Desktop Mate、VPet、BongoCat、Mate-Engine、Live2DViewerEX、Codex Pets）；
**(B) Agent 观测路线**——以"角色作为 AI 编码 agent 的状态显示器"为中心，2026 年是这条线的爆发年（OpenAI 官方 Codex Pets、Clawd on Desk 6.4k★、OpenPets、AgentPet、OC-Claw、LLMPET、dsh-pet 等）。
**两条线的交叉点（原创造型 + agent 状态）目前已被大量项目占据，但我们原本设想的"自己设计的角色 + agent 状态反应"并没有出现一个统治级产品**——占据最多眼球的是像素螃蟹 clawd 这类"单形象 + 多主题"的实现，原创形象这条路仍然开放，但时间窗口在收窄。

---

## 1. 平台 / 框架类清单（第 1 问）

| 名称 | 链接 | 类型 | 状态 / star（2026-10-09） | 形象机制（素材格式） | 商业模式 | 备注 |
|---|---|---|---|---|---|---|
| **VPet 虚拟桌宠模拟器** | https://github.com/LorisYounger/VPet ; Steam app 1920960 | 开源桌宠软件（WPF/C#） | 6,922★，最后提交 2026-10-03，**活跃**；Steam 免费 | **PNG 动画帧**（32 种 × 4 状态 × 3 种类）；ModMaker 导出；代码插件可扩展 l2d/spine 渲染 | Steam 免费 + DLC（「VPet - MOD 制作器」「Pancake 猫猫 皮肤包」）+ 创意工坊 | Steam 评价约 5.19 万条「好评如潮」（搜索摘要）；ModMaker 独立仓库 100★，2026-08-31 更新；有 339 项工坊合集 |
| **VPet.ModMaker** | https://github.com/LorisYounger/VPet.ModMaker | MOD 制作器 | 100★，2026-08-31 更新 | 图片帧生成工具 | Steam DLC / 免费 | 接入原创角色的官方入口 |
| **BongoCat** | https://github.com/ayangweb/BongoCat | 跨平台桌宠（2026 已重写为 Rust，含 crates/、Cargo.toml） | **23,869★**，最后提交 2026-10-08，**非常活跃**；2025-03 创建 | **Live2D 模型**（自带模型；README：「Bring your own Live2D models」） | 完全开源免费，无内购 | 摇尾巴猫起源 Bongo-Cat-Mver；社区模型库 Awesome-BongoCat 2,120★（2026-09-18 更新）；有 QQ 群与 B 站运营 |
| **DyberPet 呆啵宠物** | https://github.com/ChaozhongLiu/DyberPet | 开源桌宠框架（PySide6/Python） | 995★，2026-09-25 更新，**半活跃**；v0.10.3 | **PNG/GIF 帧 + JSON 配置**（docs/art_dev.md 明确规范：角色、物品、迷你宠物、音效） | 开源免费；AI 助手模块开发中，「LLM 模块相关能力暂未完全开源」 | 官方定位「让喜欢的角色住进桌面，模组自由，AI 相伴」；已有二创生态（原神角色包 314★） |
| **Shimeji / Shimeji-ee** | https://kilkakon.com/shimeji/（原版站）；分支：DalekCraft2/Shimeji-Desktop、LavenderSnek/ShimejiEE-cross-platform、Valkryst/VShimeji | 老牌桌宠（Java） | 原版长期由 kilkakon 维护（**未能核实 2026 年是否有新版**）；分支活跃：DalekCraft2 52★（2026-10-06，JRE6→JDK25 移植）、LavenderSnek 99★（v2.1.0，已合并 Kilkakon v1.0.19）、wl_shimeji 205★（Wayland，C）、Shijima-Qt 200★ | **PNG 图片集 + XML 动作/行为**（conf/actions.xml、behaviors.xml，图片文件夹名须与 img/Shimeji 一致） | 免费 / 素材在 BOOTH 上售卖替换包 | 生态靠分支续命；日文素材包（「ドットしめじ」等）在 BOOTH 标价出售 |
| **Mate-Engine** | https://github.com/shinyflvre/Mate-Engine ; Steam app 3625270 | 开源 3D 桌宠（Unity，VRM） | GitHub 3,750★，但**主仓最后提交 2026-01-20（近 9 个月未更新）**；Steam 版仍更新（X3.4，8 月 24 日，据搜索摘要）；Linux 移植 319★ | **VRM**（0.x 用 MToon、1.x 用 MToon10，需 humanoid 骨骼）+ 自有 .ME 文件；Steam 创意工坊 | GitHub 免费 + Steam 付费（$3.99 促销 / $5.49 定价，约 ¥26 国区）+ Steam 独有饰品/事件台词 | 自我定位「Desktop Mate 的免费替代」；默认形象 Zome 有版权（Yorshka Shop），**不能随构建分发**——原创角色生态的典型版权教训 |
| **Desktop Mate** | https://store.steampowered.com/app/3301060/ | 商业 3D 桌宠平台（infiniteloop） | 2025-01-08 上线；**本体免费**；2026-06-24 宣布累计 **200 万下载**；同时开 Mac 版开放测试 | **官方授权 3D 模型 DLC**（每个角色独立动作/语音）；**不支持自定义 VRM，后期版本移除了 MOD 支持**（Mate-Engine README 对比表 + Steam 评测摘要，官方页面未能直连核实） | 免费本体 + 角色 DLC 每款 ¥2,200（约 $15），2026 夏促 22 款 DLC 首次 8 折 | 角色阵容：初音ミク、镜音双子、东方、三丽鸥、Kizuna AI、NieR 2B 等；Steam 评价由「极度好评」转为**褒贬不一**（约 8,926 条、66.42% 好评，2026-03 快照，搜索摘要），槽点为 DLC 贵、功能少、移除 MOD |
| **Live2DViewerEX** | https://store.steampowered.com/app/616720/ | 商业 Live2D/Spine 桌面显示器 | 2026 仍更新（26.6.x，2026-06 升级 Cubism SDK 5r4.2、新增 Spine 4.3 支持）；$4.99 + Spine 扩展 DLC | **Live2D moc3 / LPK / JSON**，创意工坊；Spine 模型需买扩展 | 买断制 + DLC；移动版另售/看广告 | 版权 2026 Pavo Studio；官方明说「价格会随 Live2D SDK 授权费调整」——**Live2D 授权成本是这条路线的结构性成本** |
| **OpenPets** | https://github.com/OpenPetsHQ/openpets | 桌宠**平台**（插件 SDK v3） | 1,274★，2026-10-09 提交，**活跃**（2026-05 创建） | 自有 pet 格式（packages/pet-format，manifest + 动画帧），Pet Gallery 预览；插件沙盒 JS/TS | 开源免费，README 明言「Users are not the business — 永不对核心功能/插件 SDK/agent 集成收费」 | 内置 8 个官方插件（番茄钟/提醒/心情打卡/虚拟宠物）；官方 agent 集成 Claude Code / OpenCode / Cursor / Pi / MCP，**且不把 prompt/代码/路径泄漏到气泡里** |
| **Codex Pets（OpenAI 官方）** | 见第 4 节 | 官方功能内置于 Codex 桌面应用 | 2026-05 上线（gigazine 2026-05-07 等报道） | **精灵图 atlas**：v1 1536×1872（8×9）、v2 1536×2288（8×11），单格 192×208；本地 `~/.codex/pets/<id>/` 存 `pet.json` + `spritesheet.webp` | 随 Codex 免费；官方办过创作比赛（奖品为 ChatGPT Pro 30 天） | 「/hatch 上传一张图生成宠物」+ 官方 hatch-pet skill；**这是目前门槛最低的「个人专属角色」路径** |
| **Clawd on Desk** | https://github.com/rullerzhou-afk/clawd-on-desk ; https://clawdondesk.com/ | 像素桌宠（agent 状态） | **6,400★**（2026-03-18 创建，7 个月），2026-10-09 提交，**非常活跃** | **主题包**：theme.json + SVG/GIF/APNG/WebP/PNG；最小可行主题 = 1 张带眼神跟随的 SVG + 7 张状态动画；支持导入 Codex Pet zip 精灵图；scripts/create-theme.js 脚手架 | 源码 AGPL-3.0，**但美术资源不在 AGPL 内**（各版权方保留）；官方主题 Hash Sage、鲸鱼娘放在独立仓库 clawd-themes | 支持 27+ 种 agent（Claude Code、Codex、Cursor、Gemini、Kimi、Qwen、DeepSeek Harness……）、权限气泡、SSH 远程转发、PWA 手机查看——**是本赛道目前最完整的「agent 状态桌宠」参照物** |
| **DesktopMate 之外的传统显示器** | Wallpaper Engine 等（未深挖） | 壁纸/桌面层 | 未调研 | 支持 Live2D/3D | 买断 + 工坊 | 作为「角色另一个分发渠道」值得后续单独看 |

---

## 2. LLM / AI 陪伴类桌宠（第 2 问）

### 2.1 国外 / 开源

| 名称 | 链接 | 形态 | 状态 | 形象机制 | 商业模式 | 口碑 / 备注 |
|---|---|---|---|---|---|---|
| **Open-LLM-VTuber** | https://github.com/Open-LLM-VTuber/Open-LLM-VTuber | 语音对话 AI 伴侣 + Live2D 桌宠模式（Electron 前端） | **14,025★**，主仓最后提交 2026-05-15（**约 5 个月未提交**）；README 称 v2.0 全量重写规划中 | Live2D（Cubism 5）；形象需自备/自寻授权 | 开源免费（自备模型 API） | 功能最全的开源参照：视觉感知、MCP 工具、Letta 长期记忆、多角色群聊；桌面模式支持透明、置顶、点击穿透 |
| **my-neuro** | https://github.com/morettt/my-neuro | AI 桌宠 + 1 秒语音对话 | 1,390★，2026-10-05 更新，活跃 | Live2D（兼容多种定制） | 开源免费 | 长期记忆、视觉识别、声音克隆、LLM 训练 |
| **Soul-of-Waifu** | https://github.com/jofizcd/Soul-of-Waifu | 桌面角色扮演 / AI 伴侣 | 1,381★，2026-08-28 更新 | Live2D / VRM + 本地 LLM | 开源免费 | 「给喜欢的角色一个灵魂」，RPG 玩法 |
| **Artemis** | https://github.com/momori777/Artemis | 本地 AI 女友全家桶（含 Live2D 桌宠、酒馆角色卡导入） | 379★，2026-09-30 更新 | Live2D + 可导入 SillyTavern 角色卡 | 开源免费（本地 8G 显存可跑） | 可接 Claude Code / OpenClaw 做 Agent 能力 |
| **fount** | https://github.com/steve02081504/fount | 角色/人格运行框架（可接桌宠前端） | 691★，2026-10-09 更新 | 多前端 | 开源 | 中文社区项目 |
| **MiniCPM-Desk-Pet** | https://github.com/OpenBMB/MiniCPM-Desk-Pet | 端侧小模型桌宠 | 492★（2026-05 创建），2026-08-18 更新 | 未核实 | 开源 | OpenBMB 出品，local-first |
| **AI Desktop Pet (42kami)** | Steam app 4227700 | 付费 AI 桌宠 | 2026-04-28 上架，$12.99 | **VRM / Spine / 静态图** + Steam 工坊 | 买断 $12.99 | 内置本地 LLM、可看屏、语音克隆；第三方估算约 68 评价 / 约 $1.7 万收入（games-stats / Datahumble 估算，**未证实**） |
| **Gulugulu** | Steam app 4956830 | 桌面鸭养成小游戏 + 可选接编码 agent | 2026-08-18 上架，$1.99 | 像素 | 买断 $1.99 | **把 Codex/Claude 会话的 token 消耗变成宠物的食物和经验**——与我们核心卖点最近的商业化先例 |
| **Project N.E.K.O.** | Steam app 4099310 | 免费 AI 伴侣桌宠（EA） | 免费 EA，计划 2026 年中完成 EA（据搜索摘要） | **Live2D / VRM（+MMD）** + 工坊（角色、人格模板、语音包、服装） | 免费（计划 EA 后仍免费） | 目标含「Neko Network」AI 社交网络；Steam 工坊直接收角色模型 |
| **Desktop Mate** | 见上表 | 无 LLM 的 3D 陪伴 | 200 万 DL | 官方 DLC 模型 | DLC | 证明「陪伴感」不需要对话也能卖 |
| **Neuro-sama**（参照） | twitch/yt | 闭源 AI VTuber | 持续运营 | 自研 3D | 直播打赏 | Open-LLM-VTuber 即其开源复刻 |

### 2.2 国内商业产品

| 名称 | 归属 | 形态 | 2026 状态 | 形象机制 | 商业模式 | 口碑 / 备注 |
|---|---|---|---|---|---|---|
| **逗逗游戏伙伴 / Hakko AI** | **心影随形 LynkSoul**（不是腾讯自研；应用宝只是分发渠道） | 桌宠 + 悬浮球，AI 游戏陪伴（实时语音、VLM 看屏） | 2026 仍更新（应用宝版 8.0.18，Win10 版 2026-06） | 原创 IP 角色（妮可、妮卡）+ B 站 UP 主数字分身 | 免费 + 会员（体力/时长/长期记忆与月卡挂钩） | 全球注册用户破 1000 万、MAU 百万级（搜索摘要：虎嗅 2025-10 称 MAU 200 万+）；Google Play 2.8/5（1290 评价）——**吐槽集中在付费墙、识别不准、"AI 太呆"** |
| **QQ 宠物（2026 回归）** | 腾讯 | 手机 QQ 内 3D 宠物 | **2026-07-27 正式回归**（停运 8 年后） | 3D 形象 + 可选形象/性格 | 免费（生态型产品） | 接入腾讯混元 Hy3；**去掉生病/死亡机制**；被视为「桌面养成 + 大模型」品类升温的标志 |
| **SentiCat** | SentiPulse 思维光谱（北京，2025-09 成立） | 桌面 AI 小猫 + 数字伙伴 SUSU | **2026-04-23 开启公测**，仅 Windows | **Live2D 形象 SUSU**（可换装扮、有情绪状态） | 公测发放 Token 额度，之后充值 | 「任务执行 + 情感陪伴」双核；接入 7 家国产模型（MiniMax/通义/智谱/豆包/DeepSeek/Kimi/小米）；本地 SQLite 存储；内置插件市场与 MCP |
| **米哈游《BSide: Olivia Lin（林离）》** | 米哈游稻海桑田 | 桌面 AI 陪伴 + 音乐生成 + 动态壁纸 | **2026-07-13 上架 EA，2026-08-11 宣布停运，2026-08-31 下架——上线 29 天** | 官方 3D/2D 角色「林离」 | 未及变现即停 | **Steam 1500+ 评价、82% 特别好评仍停服**：本赛道 2026 年最强的负面信号 |
| **AnuNeko** | Anuttacon（蔡浩宇） | 实验性 AI 聊天（默认黑猫形象） | 2025 年底上线，**8 个月后永久关停** | 猫形象（两种模型） | 免费 | 官方理由：团队规模有限、资源转移 |
| **慵懒小猫 / 快乐小狗** | 独立开发者 | iOS 桌面宠物 + 小组件/灵动岛 | 2026 在架 | AI 形象（快乐小狗可上传自家狗照片生成专属狗） | 免费/内购（未核实） | 面向普通用户的轻量路线 |
| **LOOI** | LOOI | 手机 + 桌面机器人底座 | 国区 App Store 在架 | 机器人本体 | 硬件销售 | 与桌宠软件不同的硬件路线 |
| **超级 QQ 秀 / QQ 小窝** | 腾讯 | 虚拟社交 | **2026-08 停运** | – | – | 被新版 QQ 宠物替代 |

### 2.3 一句话总结第 2 问

「能对话的桌面伴侣」在 2025–2026 已经**严重同质化**：开源侧是 Live2D + LLM + TTS 的标准组合（Open-LLM-VTuber 14k★ 为标杆），商业侧要么靠大厂生态（QQ 宠物、逗逗），要么靠低价买断（Gulugulu $1.99）或 Token 收费（SentiCat）。
**目前没有一家用"原创角色 + 编码 agent 状态"这个组合站住脚**——这正是我们产品定义里的空白。

---

## 3. 普通人如何获得「自己的专属角色」（第 3 问）

### 3.1 委托画师 / 建模师（成熟先例，成本明确）

| 路径 | 价格区间（来源与日期） | 说明 |
|---|---|---|
| Live2D 建模（自备拆件立绘） | Q 版全身 NT$4,000 起；正比全身 NT$8,000–15,000 起；精致方案 NT$20,000 起（clibo.tw 委托页，2026 在挂） | 台湾绘师平台报价 |
| Live2D 一条龙（设计+立绘+拆件+建模） | Q 版 NT$5,000–8,000 起；半身 NT$15,000 起；正比全身 NT$20,000–25,000 起，最高见 NT$35,000–50,000 / NT$60,000 | 交付周期约 4 周–3 个月 |
| 日本 nizima 官方定制 | 轻量 5 万日元 / 基础 15 万日元 / 高级 25 万日元起（不含立绘） | Live2D 官方市场，平台抽成 10% |
| 现货 Live2D 模型 | nizima 上 47.5% 商品定价 300–10,000 日元，14.6% 超 10 万日元（**2024-09 统计**）；BOOTH 通用模型 ¥2,000–6,500；限定 1 点款 ¥55,000–90,000 | 买现成比定制便宜一个数量级 |
| VRM（3D） | **VRoid Studio 完全免费**（2026 v2.11.0，可导 VRM 1.0）；BOOTH / VRoid Hub 有 0 日元成品；素质好的量产模型 5,000 日元+；「VRoid 打底 + 外包贴图/绑定」约 $100–500（VIVERSE 2026 定价文） | 3D 路线的入门成本几乎为 0，代价是画风同质（VRoid 脸） |
| 国内定制（Codex 桌宠） | 闲鱼：模板 ¥10；互动工具 ¥十几；**桌宠定制服务 ¥99–200**（知乎实测文，2026） | **中国已经出现桌宠定制生意**；文章同时提醒「只发一张图 / 只帮安装」不算完整交付，完整交付=形象+九组动作+精灵图拼接+检查+本地安装 |

### 3.2 AI 生成路径（2026 新变量）

- **OpenAI Codex Pets `/hatch`**：上传一张图，由 Codex 生成整只动画宠物（本地 `pet.json` + `spritesheet.webp`），社区已有 `hatch-pet` skill、npm 画廊（@legeling/codex-pet）、以及 Hugging Face 上的 `Codex-Pets-Sprite-Sheet-Generator`（单图 → V2 精灵图）。
- **dsh-pet（1,184★，2026-08 创建，中文项目）**：把「AI 生成动画的提示词配方 → 源视频转透明动画的管线 → 可直接用的宠物插件」三件套开源，**任何人可以从零生成自己的桌面宠物**（换角色、换动作、换风格，全流程可复现）。这是目前把「AI 生成专属角色」做到工程化最完整的一例。
- **BongoCat / VPet 等平台**：本身不提供 AI 生成，但接受标准 Live2D / PNG 动画，可以承接 AI 生成后再人工修整的素材。
- 对「原创角色」的定义影响：AI 让**形象资产的边际成本趋近于 0**，但同时让「随机 AI 脸」泛滥——差异化会更多地落到「人格 / 与 agent 的绑定关系 / 动画演出」上。

### 3.3 资产商店

BOOTH（Pixiv，日本独立创作中心，VRM/Live2D 都卖）、**nizima**（Live2D 官方，抽成 10%，可预览动作）、VRoid Hub（免费 VRM 授权下载，需查商用标签）、Steam 创意工坊（VPet / Mate-Engine / MateEngine / Live2DViewerEX / Project N.E.K.O. / AI Desktop Pet 都在用）。
**结论：让个人拥有专属角色在 2026 年是成熟生意**，预算可从 ¥0（VRoid 自捏 / 免费模型 / AI 生成）到 ¥数百（BOOTH 现货、国内代做）到 ¥数千–数万（委托一条龙）。**没有任何平台能把"我的角色"跨产品带走**——这是跨平台的空白。

---

## 4. 「角色 × 开发者工作流」的结合（第 4 问）

这是 2026 年最热的一格。按"是否与我们直接竞争"分组：

### 4.1 直接同类（AI 编码 agent 状态桌宠）

| 名称 | star / 状态 | 形象机制 | 亮点 | 商业模式 |
|---|---|---|---|---|
| **Clawd on Desk** | 6,400★，2026-10-09 活跃 | 主题包（SVG+GIF/APNG），3 内置主题 + 2 官方可选主题 + 支持导入 Codex Pet zip | 27+ agent 适配、权限审批气泡（Claude Code/Codex 可回传 allow/deny）、多 subagent 杂耍动画、SSH 远程状态转本地、PWA 手机端 | AGPL 源码免费；**美术资源独立授权**（官方主题另仓）——开源核心 + 内容分发的变现结构 |
| **OpenPets** | 1,274★，活跃 | 自有 pet manifest + 插件 SDK v3 | 明确的"平台"定位：宠物商店式 Pet Gallery、插件权限/配额沙盒、8 个官方插件；明确不 paywall | 开源（"Users are not the business"） |
| **AgentPet** | 375★，2026-10-09 活跃 | 自研形象 + 成长系统 | 监控 Claude Code/Codex/Cursor/Gemini，**token 喂食 → 升级 → 排行榜**（游戏化） | 未核实 |
| **OC-Claw** | 355★，2026-06-17 后未提交 | **自定义角色动画（"OC = Original Character. Make it yours."）**，可给不同 agent 配不同角色 | macOS 刘海 / Windows 任务栏形态、SSH 连远程 agent | oc-claw.ai 官网（未核实收费） |
| **LLMPET** | 未取 star | 盯着 Claude Code 的桌宠 | 表情 + 消息气泡 + **一键授权** + token 用量与花费统计；MIT | 免费 |
| **claude-pet (nikhilagrima)** | 未取 star | 11 种 SVG 情绪 | Claude Code hooks 驱动 + **GitHub 仓库监视（commit/PR/Actions 通过 ✓ 失败 ✗/部署状态）** | 免费 |
| **dsh-pet** | 1,184★ | 100+ 手绘透明动画 + **AI 视频自造宠物素材链** | 中文生态（DeepSeek Harness 插件）；余额档位动画、碎碎念、多开 | 免费 |
| **Foxbell (dsh-foxbell-pet)** | 未取 star | 小狐狸 | 多项目状态卡片（红=待审批、黄=运行中、绿=完成未读、深红=错误断联）+ 语音提醒 | 免费 |
| **cc-haha** | 14,932★ | 内嵌 "task-aware desktop pets" | 主体是 Claude Code 桌面工作区，桌宠是附属功能——说明**桌宠正在被吸收成 IDE/工作区的一个模块** | 开源 |
| **Bankr Buddy** | 未取 star | 复古 CRT 像素电视 | 本地 HTTP（默认 23444）接收 JSON 状态，明说可接 **CI/CD 与监控看板**；状态优先级 error > working > thinking > happy > sleeping | 未核实 |
| **FECONF 2026 mascot "Buddy"** | npm 包 | 像素蜗牛 | `npx mascot-watch npm run build` 包装构建命令，构建/测试/类型检查/dev-server 状态 → 表情与气泡，附带 Web Vitals（LCP/INP/CLS）反馈；本地 webhook 127.0.0.1:7842 | 会议吉祥物（免费） |
| **OpenAI Codex Pets** | 官方功能 | 8 个内置像素宠物 + `/hatch` 自定义 | 悬浮 overlay、最小化也能看进度（running / waiting / review）、**点宠物可直接回复 agent** | 随 Codex 免费——**平台方亲自下场，这是最大的竞争变量** |

### 4.2 相邻：系统监控 / 构建状态拟人化（更早的传统）

- **Whaly**：蓝色小鲸桌宠，底部常驻 CPU/内存/磁盘/GPU/DRAM 环形图与趋势线，右键模拟高负载（鲸鱼发热、屏幕泛红），自我定位"开发者工具链里的彩蛋"。
- **live2dpet**：Live2D + LLM/TTS，psutil 采集硬件状态与前台窗口，低电量/高负载时宠物主动搭话（Python + PySide6 + ZeroMQ 四进程架构）。
- **Hopet**（分析文）：总结出这类产品的本质——「用户真正想知道的不是 CPU 占用，而是 agent 是否安好」，把 pending/running/error/completed 映射为情绪表情，利用人的拟人化倾向。
- **历史脉络**：从系统监控挂件 → 桌面宠物 → agent 状态伴侣，三条线在 2026 年合流。

**结论（第 4 问）**：把角色与开发者工作流结合**已经有大量先例**，且 2026 年上半年集中爆发；但绝大多数是像素/简单形象 + 单角色多主题，**真正"投入美术成本做原创角色演出"的极少**（Clawd 的鲸鱼娘、哈希仙人是少数例子）。差异化空间在「角色本身的可辨识度与情感演出」，而不是"能显示 agent 状态"这个功能点本身。

---

## 5. 赛道成败信号（第 5 问）

### 5.1 正向信号（升温）

1. **Desktop Mate 累计下载 200 万**（2026-06-24 官方公布，Mac 版开放测试）——2025 年初才上线，持续新增 22 个 DLC 角色（含三丽鸥、初音系、Kizuna AI）。
2. **QQ 宠物 2026-07-27 回归**，接入混元 Hy3、全面 3D 化、去掉生病死亡机制——大厂用 AI 重做"桌面养成"。
3. **BongoCat 从 2025-03 建立到现在 23,869★**，2026 年仍在每日提交；社区模型库 Awesome-BongoCat 2,120★。
4. **OpenAI 官方在 Codex 里内置宠物**（2026-05），并举办创作比赛——agent 厂商认可"宠物是状态 UI"。
5. **Clawd on Desk 7 个月 6,400★**，被 awesome-claude-code 收录；同类项目（OpenPets/AgentPet/OC-Claw/dsh-pet）在 3–8 个月内密集出现。
6. **闲鱼已出现桌宠定制生意**（¥99–199 代做 Codex 桌宠）——需求外溢到付费服务的信号。
7. **Gulugulu**（$1.99，2026-08-18）把 token 消耗变成宠物食物，验证了"编码 agent 使用量 → 养成"的小额付费模型。

### 5.2 负向信号（降温 / 失败）

1. **米哈游《BSide: Olivia Lin》上线 29 天停服下架**（2026-07-13 → 08-31），Steam 1500+ 评价、82% 好评也救不了——**"角色 + AI 陪伴"不等于能活**，算力成本与留存是大坑。
2. **AnuNeko（蔡浩宇系）运营 8 个月关停**。
3. **腾讯超级 QQ 秀 / QQ 小窝 2026-08 停运**（虚拟社交让位于新版 QQ 宠物）。
4. **Desktop Mate 口碑滑坡**：从"极度好评"到"褒贬不一"（约 8,926 条 / 66.42%，2026-03 快照），槽点=**角色 DLC 太贵 + 移除 MOD 支持 + 交互贫乏**——正好是"封闭角色平台"的三个雷。
5. **多个开源项目进入停滞**：Mate-Engine 主仓 2026-01 后无提交、Agentic-Desktop-Pet 2026-01 停更、OC-Claw 2026-06 后停更、Open-LLM-VTuber 主仓 2026-05 后无提交（重写中）。
6. **AI 陪伴产品的共同吐槽**：三日新鲜感、付费墙（逗逗的时长/体力限制）、识别不准（"AI 太呆"）。

### 5.3 交叉结论

**活下来的是"轻资产、快迭代、免费或低价 + 平台化"的产品**（BongoCat、VPet、Clawd、OpenPets、Desktop Mate 靠 DLC 摊成本）；
**死掉的是"重资产、单角色、重情感承诺"的产品**（米哈游林离、AnuNeko）。
对我们最重要的一条：**单纯"有角色"不值钱，"角色 + 具体场景的独占价值"才值钱**——我们的具体场景就是 Claude Code 的工作状态。

---

## 6. 对本产品的直接可借鉴点（简短）

1. **形象格式选型**：精灵图路线（Clawd/Codex Pets/dsh-pet）成本最低、迭代最快；Live2D 表现力最好但绑定 + 授权成本高；VRM 在 3D 陪伴里已是事实标准（Mate-Engine/N.E.K.O.）。若 v1 只做 agent 状态反应，**建议借鉴 Clawd 的"1 张带眼神跟随的静态图 + 7 张状态动画"最小主题规范**——这正是"原创角色最低可行成本"的行业答案。
2. **避开 Desktop Mate 的雷**：封闭角色平台会在"贵 + 不能改"上翻车；Clawd 的"AGPL 源码 + 美术资源独立授权 + 官方主题另仓分发"是更安全的开源变现结构，但需注意**默认模型不能随构建分发**（Mate-Engine 的教训）。
3. **agent 状态映射已被验证**：thinking/typing/subagent/权限审批/完成庆祝/休眠 这套状态机在 Clawd 已被用户接受；权限气泡（allow/deny 回传）是超出"展示"的第二步，且 Claude Code 的 HTTP permission hook 是可行路径。
4. **差异化不在"能看状态"，而在角色本身**：现有玩家几乎清一色像素 crab/猫/鸭/狐狸；自绘一个有记忆点的原创角色 + 与 Claude Code 工作流的深度绑定（如 token 消耗可视化、任务完成演出）仍有位置。
5. **合规/成本提示**：Live2D SDK（Cubism Core）授权与「模型不可再分发」是反复出现的坑；OpenAI Codex Pets 已经占据了"官方宠物"位置，正面比拼功能不现实，**应主打"你自己的角色"**。

---

## 7. 「未找到」清单（如实标注）

1. **未找到**任何已存在的、明确以「自研原创角色 + Claude Code 工作状态差异化形象变化」为核心卖点并有可观用户量的产品（最接近的是 Clawd on Desk 的鲸鱼娘主题、OC-Claw 的自定义角色，但都不是"角色即产品"）。
2. **未找到**中文市场「桌宠角色委托」的淘宝/站酷等平台系统性报价（只有闲鱼 ¥99–199 的 Codex 桌宠代做，来自单篇知乎文章）。
3. **未找到** VPet 创意工坊的项目总数、订阅数官方统计（只有"某合集含 339 项"）。
4. **未找到** Desktop Mate 官方对"是否支持自定义 VRM / 为何移除 MOD 支持"的正式声明（唯一依据是 Mate-Engine README 的对比表与 Steam 评测摘要，属竞争方口径）。
5. **未找到** "Phase Pal" 的独立页面或 2026 年状态（仅在 Mate-Engine 对比表里作为 Playtest Alpha 出现）。
6. **未找到** DeskPet / 桌宠类产品在 2026 年"关服 / 停运"的更完整清单（只找到米哈游系、腾讯虚拟社交三条；国内小厂桌宠 App 的下架情况无公开统计）。
7. **未找到** 中国「AI 桌宠」在微信小程序 / 应用商店里的完整名录（仅覆盖到被媒体报道过的 5–6 款）。
8. **未找到** Codex Pets 的官方文档页（只能通过 gigazine / Mashable / DigitalTrends 等二手报道确认功能与精灵图规格）。

## 8. 不确定项（未证实 / 需复核）

| 事项 | 现状 | 建议复核方式 |
|---|---|---|
| Desktop Mate 的 Steam 当前评价数与好评率 | 只有 2026-03 快照（8,926 条 / 66.42%） | 直连 Steam 商店页 |
| Desktop Mate 的 DLC 单价是否统一 ¥2,200 | 多来源一致（初音ミク、镜音、雾雨魔理沙），但未见官方价目表 | Steam DLC 页 |
| VPet Steam 评价数 ~51,940 | 来自搜索摘要 | Steam 页 |
| Mate-Engine 的 Steam 售价（$3.99 / $5.49 / ¥26） | 三个来源不一致，可能为不同时间/区域 | Steam 页 |
| Desktop Mate 内部是否用 VRM | **未证实**（Mate-Engine 称其"不支持自定义 VRM"，未说明底层格式） | 官方 Q&A |
| AI Desktop Pet 的销量与收入（~68 评价 / $17k） | games-stats / Datahumble 的模型估算 | Steam 页 |
| 逗逗游戏伙伴的用户量（1000 万注册 / MAU 200 万） | 来自媒体与第三方分析 | 官方披露 |
| 山鲸/哈希仙人等 Clawd 官方主题是否收费 | 未核实（仓库说明只给了授权方式） | clawd-themes 仓库 |
| nizima 的价格分布统计 | 数据日期为 2024-09-11 | nizima 站内现查 |
| Shimeji 原版 2026 年是否仍有官方更新 | 未确认（只有分支在更新） | kilkakon.com/shimeji/ |
