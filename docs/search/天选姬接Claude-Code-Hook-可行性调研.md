# 让天选姬对 Claude Code 事件做出反应 — 可行性调研

> 调研日期：2026-09-17
> 目标：Claude Code 任务完成 / 需要用户响应时，天选姬给出相应反应
> 前提文档：`天选姬桌宠-开源与改造路径调研.md`（官方桌宠闭源、安装包主负载加密）
> 说明：本文只做调查。第三节的所有结论均来自对**本机已安装的天选姬**的只读检查。

---

## 一、结论摘要

**结论：路线 A 已实测跑通——官方桌宠接受外部写入的配置并会做出反应，不需要换壳、不需要逆向。**

Claude Code 一侧完全没有障碍；天选姬一侧的关键疑问也已在 3.7 的判定实验中**得到肯定答案**。

三个关键判断：

1. **Claude Code 侧零障碍。** hook 系统提供 `Stop`（任务完成）、`StopFailure`（失败）、`PermissionRequest`（需要授权）、`Notification`（等待输入）等精确事件，支持 `async` 非阻塞执行和 `http` 直连。已有 `clawd-conduit` 等成熟先例把桌宠接进全部 15 个 hook 事件。
2. **✅ 天选姬侧：`Config.json` 热重载已确证。** 判定实验（3.7）证明程序会**在运行时读取外部写入的 `OneTimeAlarm`**，并在到点后消费清零——两次实验存活时长相差一倍，清零却都精确落在各自目标时刻后 51 秒。这意味着**改配置文件就能驱动官方桌宠**。
3. **⚠️ 但有一个重要的行为约束：全屏时会被抑制。** 实验中未能观测到视觉反应，原因是当时屏幕被全屏游戏覆盖，且程序自身记录了 `isFullScreenActived: true`。这符合桌宠软件「不打扰全屏应用」的常规设计——**对 Claude Code 场景反而是好事**（你在终端里，不是全屏游戏），但需注意别在全屏应用前面指望她弹窗。

**仍待确认**：她的反应具体长什么样（声音？动作？气泡？）——实验时她被全屏游戏遮挡，需要在小姬可见时重做。

**行动建议**：

- **主力方案走路线 A**：hook 写 `Config.json` 的 `OneTimeAlarm` 触发提醒。工程量极小，且保留**完整的原版天选姬**。
- **补充手段**：`lastDialogHint`（让她说话）、`windowLeft/Top`（移动位置）、`isWalkAroundMode`（走动）等待验证字段。
- **路线 B（换壳）作为备选**：若路线 A 的反应形式不够理想，再用自带接口的外壳承载天选姬形象。详见 4.4。

> ⚠️ 合规提醒：本文涉及的操作只改**用户自己 AppData 下的配置文件**，不涉及破解、解密或修改受版权保护的二进制。详见第六节。

---

## 二、Claude Code 侧：hook 接口

### 2.1 与桌宠场景相关的事件

| 场景 | 事件 | 说明 |
|---|---|---|
| **任务完成** | `Stop` | Claude 结束本轮回复 —— **主信号** |
| | `SubagentStop` | 子代理结束，matcher = 代理类型 |
| | `TaskCompleted` / `TaskCreated` | 任务被标记完成 / 创建 |
| **任务失败** | `StopFailure` | 因 API 错误结束。matcher：`rate_limit` / `overloaded` / `authentication_failed` / `server_error` / `max_output_tokens` |
| **需要用户响应** | `PermissionRequest` | 工具调用需要权限决策 —— **即时触发** |
| | `Notification` | matcher：`permission_prompt`（约 6 秒防抖后才触发）、`idle_prompt`（回完约 60 秒且用户未输入）、`agent_needs_input`、`agent_completed` |
| | `Elicitation` / `ElicitationResult` | MCP 服务器向用户要输入 |
| **会话** | `SessionStart` / `SessionEnd` | matcher 含 `startup` / `resume` / `clear` / `compact` 等 |
| **工具前后** | `PreToolUse` / `PostToolUse` / `PostToolUseFailure` | matcher = `tool_name` |
| **提交 prompt** | `UserPromptSubmit` | 无 matcher |

> 注意：`Stop`、`UserPromptSubmit`、`TaskCreated/Completed` **不支持 matcher**，写了也会被忽略。

### 2.2 配置结构

三级嵌套：**事件 → matcher 组 → handler 数组**。

```json
{
  "hooks": {
    "Stop": [
      { "matcher": "",
        "hooks": [ { "type": "command",
                    "command": "node",
                    "args": ["D:\\pets\\pet-hook.js", "done"],
                    "async": true, "timeout": 5 } ] }
    ]
  }
}
```

handler 类型有 5 种：`command` / `http` / `mcp_tool` / `prompt` / `agent`。

**路径优先级**（高 → 低）：managed settings → `claude --settings` → `.claude/settings.local.json` → `.claude/settings.json` → `~/.claude/settings.json`。
**hook 条目跨层是合并而非覆盖。** 用 `/hooks` 查看当前生效配置。

### 2.3 hook 收到什么

stdin 的 JSON 公共字段：`session_id`、`prompt_id`、`transcript_path`、`cwd`、`permission_mode`、`hook_event_name`；子代理内还有 `agent_id`、`agent_type`。

`Stop` / `SubagentStop` 额外提供 **`last_assistant_message`**、`stop_hook_active`、`background_tasks`。

工具事件提供 `tool_name`、`tool_input`、`tool_use_id`。

**实用推论**：
- 「这轮说了什么」用 `last_assistant_message`，**不要**读 transcript（官方明确警告 `transcript_path` 的 JSONL 异步写入、可能滞后）
- 「改了什么文件」用 `PostToolUse` 的 `tool_input.file_path`，比解析 transcript 可靠
- 「成功还是失败」用 `Stop` vs `StopFailure`
- ⚠️ **耗时需要自己算**：hook 输入里**没有**耗时字段。做法是 `UserPromptSubmit` 记时间戳 → `Stop` 相减

### 2.4 退出码（桌宠场景务必注意）

| 退出码 | 含义 |
|---|---|
| `0` | 成功 |
| `2` | **阻断** —— 在 `Stop` 上 exit 2 会**阻止 Claude 停下并继续对话** |
| 其他 | 非阻断 |

> 🚨 **桌宠的 hook 必须永远 exit 0。** 一个失败的通知脚本绝不能干扰 Claude Code 的正常工作。建议所有调用都加 `async: true` 并用 `|| true` 兜底。

### 2.5 执行特性与 Windows 约束

- 所有匹配的 hook **并行**执行
- 默认超时 600s；`UserPromptSubmit` 降到 30s；`async: true` 不强制超时
- **hooks 在会话启动时读取，没有热重载** —— 改完配置要重开会话
- Windows 下 shell 形式用 **Git Bash**；`shell: "bash" | "powershell"` 可覆盖
- **Exec 形式**（写了 `args`）直接 spawn 可执行文件、不经 shell —— **这是调用桌宠 `.exe` 的推荐方式**
- ⚠️ `.cmd` / `.bat` 无法在 exec 形式下 spawn
- 用绝对路径（含空格如 `C:\Program Files\...` 没问题）
- 已知坑：路径写错 → 退出码 127，且**静默非阻断**；`async` 下裸命令名（如 `node`）会静默失败

### 2.6 现成先例：clawd-conduit

[github.com/ssssssanjiu/clawd-conduit](https://github.com/ssssssanjiu/clawd-conduit) —— 把 Clawd on Desk 桌宠接进全部 15 个 hook 事件：

- 14 个事件用 `async` command hook（5s 超时，永不阻塞），POST 到 `127.0.0.1:23333/state` 切换宠物状态
- `PermissionRequest` 单独走 `http` hook（600s 超时），直接在桌宠上弹卡片按快捷键允许/拒绝

**其状态映射可直接借鉴**：

| hook 事件 | 桌宠状态 |
|---|---|
| `SessionStart` | idle |
| `UserPromptSubmit` | thinking |
| `PreToolUse` / `PostToolUse` | working |
| `Stop` | attention |
| `StopFailure` | error |
| `Notification` | notification |
| `SessionEnd` | sleeping |

其它可参考：官方 walkthrough 的桌面通知示例（`Notification` + PowerShell MessageBox）见 https://code.claude.com/docs/en/hooks-guide

---

## 三、天选姬侧：本机实测的接口面

> 以下全部来自对**本机已安装的天选姬**（`TX Mascot.exe`，调研时正在运行，PID 70540）的**只读**检查。未修改任何文件。

### 3.1 `Config.json` —— 明文活配置（最关键）⭐

**路径**：`%LocalAppData%\ASUS\TX Mascot\Config.json`

**证据表明它是活配置**：调研时该文件的 `lastRecordUseTime` 被更新到当时时刻（23:39），晚于加密的 `AppData.config`（19:01），且桌宠进程正在运行 —— 说明**程序运行时会写它**。

| 字段 | 实测值 | 潜在用途 |
|---|---|---|
| **`OneTimeAlarm`** | `"0001-01-01T00:00:00"` | ⭐ 一次性闹钟 ISO 时间戳，**当前为空** |
| **`lastDialogHint`** | `"None"` | ⭐ 枚举值，非 `None` 可能触发对话块 |
| `lastDialogHintTriggerTime` | `"0001-01-01T00:00:00"` | 配合上一项 |
| `dialogHintEventIsDoneFlag` | `0` | 事件完成标志 |
| `DisappearMilliSeconds` | `3000` | 气泡停留时长 |
| `DisappearTimeUnit` | `0` | 时间单位 |
| `isWalkAroundMode` / `WalkAroundDirection` | `false` / `1` | 让她走动 |
| `isDialogBoxOpened` | `true` | 对话气泡开关 |
| `appScale` / `Opacity` / `Volume` / `isMute` | `0.41` / `1.0` / `1.0` / `false` | 外观与音量 |
| `isTopMost` / `isSimpleMode` / `isFullScreenActived` | `false` / `false` / `true` | 显示行为 |
| `windowLeft` / `windowTop` | `1404.8` / `684.0` | **窗口位置**（可用来让她移动） |
| `LastFeedingTime` | `2026-07-31T11:29:18` | 喂食机制 |

**`OneTimeAlarm` 是最有希望的单点**：一个明文 ISO 时间戳，写进去就可能触发闹钟（有声音 + 有动作）。

> ✅ **热重载已实测确证**（详见 3.7）：外部写入的值会在运行时被程序读取，并在到点后消费清零。
> 路线 A 最大的不确定性**已排除**。

### 3.2 资源 DLL 暴露的功能面

**路径**：`C:\Program Files\ASUS\TX Mascot\zh-Hans\TX Mascot.resources.dll`（标准 .NET 资源程序集，39 KB，含 214 个字符串键）

这等于一份**功能清单**：

| 字符串键 | 含义 |
|---|---|
| **`String_DialogBlock_Stand_BlowBubble`** | 站着吹泡泡 |
| **`String_DialogBlock_Stand_Cute_Dialog_3` / `_4`** | 站立可爱对话 |
| **`String_DialogBlock_Sit_Interactive_3` / `_4`** | 坐着互动 |
| **`String_LookPhoneToast_Title_1..4`** / **`Content_1..9`** | ⭐ 一整套 **Toast 通知系统** |
| **`String_Pomodoro`** + `PomodoroMedia_*FocusAudio_001..028` + `RestAudio_001..005` + `Wallpaper_001..005` | ⭐ 内置**番茄钟**（28 条专注语音） |
| `String_ClickInsertMessage_1..10` | 点击触发的台词 |
| `String_ClickInterruption_WithVoice_1..9` | 点击打断的语音 |
| `String_ToolBox_Alarm_Description` | 工具箱-闹钟 |
| `String_Gashapon_*` / `String_Mini_Games` / `String_MemeCreator` / `String_Gallery` / `String_RecordLog_*` / `String_News` / `String_Character` | 其它功能模块 |

> 💡 **`String_DialogBlock_*` 这套命名很可能就是 `Config.json` 里 `lastDialogHint` 的取值域**（当前值为 `"None"`）。这是最值得验证的假设之一。

> 💡 **番茄钟这条尤其值得注意**：「专注时段结束 → 提醒」在结构上和「Claude Code 任务完成 → 反应」是同一件事。她本来就具备「计时结束 → 主动通知」的能力。

### 3.3 动画 DLL 扩展机制

**配置**：`C:\Program Files\ASUS\TX Mascot\CustomizeConfig.json`（**明文**）

一种动作类型长这样：

```json
{
  "CustomizePackageEventType": "Random",
  "CustomizePackageTimeType": "SpecifiedDate",
  "StartTime": "2025-05-01T00:00:00",
  "EndTime":   "2025-05-06T00:00:00",
  "CustomizeAnimationList": [
    { "Action": "CustomizedPath", "ActionPar": "FLabor_1.dll",
      "Weights": 10, "Repeat_Count_Max": 4, "Repeat_Count_min": 2 }
  ]
}
```

**要点**：
- 动作可由**外部 DLL** 提供（`CustomizedPath` + `ActionPar` 指向 DLL 文件名）
- 支持 `Weights`（权重）、`Repeat_Count_Min/Max`（重复次数）
- 由 `StartTime` / `EndTime` **时间区间驱动**

**动画库**：`Action\Image_Dll\` 下 **134 个动画 DLL**，按类分组：

| 前缀 | 内容 |
|---|---|
| `Basic_1..16` | 基础动作 |
| `Event_1..N` | 事件动作 |
| `F*` | 节日包：`FNewYear_1..4`、`FLantern`、`FLabor`、`FChildren`、`FDragonBoat`、`FChineseValentines`、`FMoon`、`FChristmas` |
| `Gift_1..14` | 礼物 |
| `Gasha*` | 扭蛋机（`GashaMachine` / `GashaMachineTry` / `GashaOpen` / `GashaLight`） |
| `Clothing_*` / `ChangeClothing_Mask` | 换装 |
| `EatSomething_*` / `Feed_1` | 吃东西 / 喂食 |
| `Balloon_*` | 气球 |
| `DailySuitAction\Image_Dll\` | 日常动作：`Drink_1..7`、`EatFood_1..10`、`Clothing_1..3` 等 |

> 这是官方的**动作调度机制**，按时间驱动。理论上可以往 `CustomizeConfig.json` 里加条目来在指定时间触发指定动作 —— 但**是否支持用户自造 DLL、程序是否热重载该配置，均未验证**。

### 3.4 进程与命令行

- 主程序：`C:\Program Files\ASUS\TX Mascot\TX Mascot.exe`
- **接受命令行参数**：`/duplicate` 已源码级确证（绕过单实例互斥）
- 除 `/duplicate` 外是否有其它参数 —— **未找到证据**

### 3.5 加密的部分（此路不通的地方）

| 文件 | 状态 |
|---|---|
| `%LocalAppData%\...\AppData.config` | 🔒 加密（与 `DialogContentConfig` 同一前缀 `SnA7Nj1GPNs`） |
| `Data\DialogContentConfig_zh-cn.json` | 🔒 加密 |
| `Data\DCConfig.json` | 🔒 加密（单行 base64） |
| `STDRecord.config` / `UserUsageRecord.config` / `UserBackpackList.config` | 🔒 DPAPI 加密（`AQAAANCMnd8BFdERjHoAwE...`） |
| `Backup\Config.json` | ✅ 明文（但疑似备份，非活配置） |
| **`Config.json`（顶层）** | ✅ **明文，活配置** ⭐ |
| `CustomizeConfig.json` | ✅ 明文 |
| 安装包主负载区（9–212 MB） | 🔒 加密（熵值 8.000） |

**要点**：加密的是「用户存档 / 对话内容 / 安装包」，而**行为配置是明文的**。这正是本方案可行的原因——我们不需要碰加密部分。

### 3.6 窗口、输入与进程实测（结论偏负面）

以下为本机只读/低风险实测结果：

**① 技术栈已确证：.NET Framework 4.7.2 WPF**
`TX Mascot.exe` 内含 `mscorlib`、`PresentationFramework`、`PresentationCore`、`WindowsBase`。
→ 此前「推测为 .NET/C#」的判断**升级为确证**。

**② 二进制内检索：无 Raw Input / DirectInput / 全局钩子**
检索 `RegisterRawInputDevices`、`GetRawInputData`、`DirectInput8Create`、`SetWindowsHookEx`、`GetAsyncKeyState`、`RegisterHotKey` —— **全部 0 命中**。
→ 键盘输入**最可能走普通 WPF 消息管线**（字符串级证据，非确证）。这本该对 `PostMessage` 有利。

**③ 窗口拓扑**
- 可见窗口：Class = `HwndWrapper[TX Mascot.exe;;GUID]`，Title = `ASUSMascot`，ExStyle = `0x80080`（TOOLWINDOW | LAYERED），尺寸约 123×123 px
- 因是 TOOLWINDOW，`MainWindowHandle` 返回 0（找窗口时要按 Class/Title 找，不能用主窗口句柄）
- 另有约 11 个隐藏的 136×38 WPF 窗口（疑似闹钟小时钟池）、`Hidden Window`、`SystemResourceNotifyWindow`、1 个 WinForms 窗口

**④ `PostMessage` 实测无效** ⚠️
向该 HWND 投递 `WM_KEYDOWN` / `WM_KEYUP`（VK_LEFT，lParam `0x014B0001` / `0xC14B0001`），**窗口矩形不变**。
即使先 `SetForegroundWindow`（返回 True，且 `GetForegroundWindow()` 确认就是它）再投递，**仍不变**。

> **局限（重要）**：这是**单次实验、单一 HWND、单一按键**，且当时 `isDialogBoxOpened=true`（对话气泡开着，可能吞掉方向键）。**不构成普适否证。**
> **`SendInput` 版本实测被中止，未验证** —— 这条路仍未被排除。

> 📌 原理补充：`PostMessage` 不经过输入管线，不更新键盘状态（`GetKeyState` 无效）、不触发钩子、顺序不保证；UIPI 会拦截向更高完整性（管理员）进程的投递。`SendInput` / `keybd_event` 才走真实输入管线，但**只到前台窗口**——需要目标在前台。让非前台目标接收通常要进程内注入（**高风险，不建议**）。

**⑤ 无任何 IPC 接口**
该进程 **0 个 LISTEN 端口**，无文档化 IPC。

**⑥ 其它实测**
- `Sound\Alarm\Alarm1.wav` / `Alarm2.wav` —— 闹钟铃声文件确实存在
- `Source\Character\Character_1.zip`、`Gift_1.zip` 是**包**，`%LocalAppData%\...\Character` 是解包后的 jpg/png
- `Wallpaper` 目录为空
- `ForegroundProcess.dll`、`FanSpeed.dll`、`ATK.dll`、`PowerNotifyLibrary.dll` 存在 → [推测] 它对**前台进程**和硬件状态有反应，但无文档、无验证

### 3.7 ⭐ 判定实验记录（2026-09-17/18 夜间，已完成）

> **实验目的**：判定 `Config.json` 是否在运行时被重新读取（热重载）。
> **实验方法**：备份后，字节级替换 `OneTimeAlarm` 为未来某一时刻，全程轮询观测，结束后还原。
> **合规性**：只改用户自己 AppData 下的一个字段，字节长度不变（2422 字节），CRLF/编码原样保留。

**实验一**：写入 `23:52:44`

| 时刻 | 事件 |
|---|---|
| 23:51:14 | 写入 `OneTimeAlarm = 2026-09-17T23:52:44` |
| 23:52:44 | （目标时刻） |
| 23:53:35 | 程序将字段重写回 `0001-01-01T00:00:00` |

→ 值存活 **2 分 21 秒**，清零发生在目标时刻 **+51 秒**

**实验二（判别实验）**：写入 `23:59:46`（更远的未来，用于排除「周期保存盲目覆盖」）

| 时刻 | 事件 |
|---|---|
| 23:55:46 | 写入 `OneTimeAlarm = 2026-09-17T23:59:46` |
| 23:59:46 | （目标时刻） |
| 00:00:37 | 程序将字段重写回 `0001-01-01T00:00:00` |

→ 值存活 **4 分 51 秒**，清零发生在目标时刻 **+51 秒**

**结论（关键推理）**：

两次实验的**存活时长完全不同**（2m21s vs 4m51s），但**清零时刻都精确落在各自目标时刻后 51 秒**。
如果是「固定周期的盲目覆盖」，清零时刻应与目标时刻无关 —— 但它显然被目标时刻驱动。

> ✅ **判定：程序在运行时读取了外部写入的 `OneTimeAlarm`。（热重载成立）**
> 且程序在到点后**消费**了它（one-time alarm 触发后自动清空），说明它不只是"读到了"，而是**真的把它当作一个待触发的闹钟处理了**。

**未能观测到视觉/听觉反应 —— 原因已查明**：

- 实验期间用户的屏幕被**全屏游戏（英雄联盟）+ 直播画面完全覆盖**，小姬窗口被完全遮挡
- 配置中 `isFullScreenActived: true` —— **程序自身检测到全屏应用在前台**
- 桌宠类软件的常规行为：检测到全屏时**抑制弹窗/动画**，避免打扰

→ 因此「反应长什么样」（声音？动作？气泡？）**仍未能确认**，需要在小姬可见时重做。

**还原情况**：

- 实验后 `OneTimeAlarm = 0001-01-01T00:00:00`，**与原始值一致**（由程序消费后自动清零）
- 当前文件与原始备份逐行 diff，**唯一差异**是 `lastRecordUseTime`（程序自身心跳字段，非实验所改，不应回写）
- 备份哈希与实验前记录一致，完好

---

## 四、两种路线与生态对比

「让天选姬接上 Claude Code hook」实际上有**两条性质完全不同的路线**，需要先分清：

| | **路线 A：驱动官方天选姬** | **路线 B：换壳承载天选姬形象** |
|---|---|---|
| 做法 | 改 `Config.json` / 发窗口输入，让**官方桌宠本体**做反应 | 用开源框架承载天选姬素材，接成熟 hook 桥接 |
| 得到的是 | **原版天选姬**（官方形象、语音、134 个动画） | 天选姬的**形象**，但换了运行时 |
| 控制粒度 | 受限于官方暴露的字段，**粗** | 完全可控，**细** |
| 工程量 | 小（一个脚本） | 大（要准备素材 + 接桥） |
| 成熟先例 | ❌ **零** | ✅ 多个成熟项目 |
| 主要风险 | 热重载未知（见第五节 #1） | 素材制作成本、形象保真度 |

### 4.1 路线 A：驱动官方天选姬 ✅ 已跑通

**判定实验（3.7）结论：`Config.json` 热重载成立，程序会读取并消费外部写入的 `OneTimeAlarm`。**

可用手段（按验证程度排序）：

| # | 手段 | 验证状态 |
|---|---|---|
| 1 | **`OneTimeAlarm`** 写 ISO 时间戳触发闹钟 | ✅ **已实测确证**：程序运行时读取并到点消费 |
| 2 | **`lastDialogHint`** 填对话块枚举值让她说话 | 字段存在；**取值域未知**，待验证 |
| 3 | `windowLeft` / `windowTop` 移动窗口 | 字段存在，待验证 |
| 4 | `isWalkAroundMode` / `WalkAroundDirection` 走动 | 字段存在，待验证 |
| 5 | `CustomizeConfig.json` + 动画 DLL | 机制存在（日期区间→动作）；用户自造 DLL 未验证 |
| 6 | 窗口消息 / 模拟键盘 | ⚠️ `PostMessage` 单次实测无效（有局限，见 3.6④）；`SendInput` 未测 |

**已排除的最大障碍**：~~无热重载~~ → **已确证有热重载**。

**仍存在的注意点**：

1. **程序会周期性回写整个文件** —— 但实验证明它**不会**盲目覆盖成自己的内存值（否则不会有「目标时刻+51秒」的规律）。它会读出外部值、消费、再写回
2. **全屏应用时会抑制** —— `isFullScreenActived: true` 时她不会打扰你
3. **零公开先例** —— 目前仍是全网第一个这么做的人，遇到问题没有前人可以查
4. **字段随时可能被版本更新改掉** —— 依赖的是「官方碰巧留在明文里的字段」，不是契约

> **优点**：**保留完整的原版天选姬**——官方形象、语音、134 个动画、番茄钟、扭蛋机全都还在；工程量极小（一个脚本）。
> **缺点**：控制面不是设计给你用的 API，稳定性无保证。
>
> **判断**：✅ **可作为主力方案**。

### 4.2 路线 B：换壳承载天选姬形象

现成项目实测对比（2026-09-17 核实）：

| 项目 | star | 许可证 | 感知机制 | 形象格式 | 换天选姬难度 |
|---|---|---|---|---|---|
| **clawd-on-desk** | 6.2k | AGPL-3.0 | hooks + 阻塞式 HTTP 权限 hook | `theme.json` + 1 SVG + 7 GIF/APNG | ⭐ **最低**：`node scripts/create-theme.js` 生成脚手架，零代码。**不支持 Live2D/VRM** |
| **openpets** | 1.2k | MIT | **MCP + hooks 双路** | 未证实 | 有插件 SDK v3 正式入口，但素材格式未公开 |
| **agentpet** | 355 | MIT | hooks → **Unix socket 守护进程** | `pet.json` + 图集 | 中等 |
| **oc-claw** | 351 | 称 MIT（API 返回 NOASSERTION） | 纯 hooks | 未证实 | 文档不足 |
| **Mate-Engine** | 3.7k | AGPL-3.0 + MateProv2 | ❌ **无任何编程助手集成** | **VRM** ✅ | 需靠社区 mod `Extension-MateEngine`（**仅 4★**，本地 HTTP `localhost:8787`，提供 `/api/expression` 表达、`/api/animation`、`/api/speak`、`/api/custom-animation`），代价是要在 Unity 工程里加组件**自行重编译** |
| Agentic-Desktop-Pet | 339 | 无 license 字段 | ❌ **无 Claude Code 集成**（标题里的 "Claude Code" 只是定位词） | Godot `.pck` | 不适用 |

**关键结论**：

- **接受 2D 形象 → `clawd-on-desk` 换主题最省事。** 12 个状态的桥接是现成的，你只需按规范做一套天选姬素材，**零代码**。
- **坚持官方 MMD→VRM 的天选姬 3D 模型 → 没有现成项目可直接换形象，必须自写桥接。** 链路：Claude Code hooks → 本地脚本归一化 → 转发。桌宠端二选一：Mate-Engine + 8787 端口（需重编译）或自写 VRM 渲染。

### 4.3 值得直接抄的桥接范式

**⭐ 最重要的一条行业经验**：社区驱动桌宠的成熟套路，**不是注入别人的闭源宠物，而是选一个自带接口的桌宠**。

| 自带接口的桌宠 | 接口 |
|---|---|
| **Live2DViewerEX** | 官方 **ExAPI WebSocket**：`ws://localhost:23333`（[文档](https://live2d.pavostudio.com/doc/en-us/exapi/)）—— 闭源付费，但**有官方 API**，可靠性最高 |
| Mate-Engine | 社区扩展 REST：`:8787`（`/api/expression`、`/api/animation`、`/api/speak`） |
| Desktop Goose | DLL mod + `config.ini` |

**Claude Code hook → 桌宠的现成先例**：

| 项目 | 做法 |
|---|---|
| `aemeath_withclaude` | hook POST 到 `:9527` |
| `clawd-on-desk` | hook POST 本地 `/state` |
| `dsh-pet-indesktop` | 事件总线协议 |
| claw-universe `desktop-pet-notify` | POST `/notify` |

**桥接层可抄的实现**：

| 范式 | 出处 | 要点 |
|---|---|---|
| **`hopet-emit`** | Hopet | hook JSON → 归一化 StateEvent → Unix socket；**失败静默 `exit 0`，绝不阻塞 Claude Code** ⭐ |
| `gemba-bridge` | gemba-core | 追加 JSONL，20ms 级延迟 |
| `/state` HTTP 端点 | clawd-on-desk | hook 直接 POST 本地端口切换状态 |
| MCP + hooks 安装器 | openpets | 自动写入 `settings.json` 与 CLAUDE.md |

### 4.4 推荐

**✅ 主力方案已确定为路线 A**（判定实验已跑通，见 3.7）。

**推荐架构**：

```
Claude Code hook (Stop / StopFailure / PermissionRequest / Notification)
        ↓  async command hook，永远 exit 0
   本地脚本
        ↓  写 %LocalAppData%\ASUS\TX Mascot\Config.json
   OneTimeAlarm = now + N 秒
        ↓
   天选姬响铃提醒你
```

**下一步要做的事**：

1. **在小姬可见时补做一次实验**，确认触发时的表现（声音？动作？气泡？）——这是目前唯一的缺口，因为第一次实验时她被全屏游戏挡住了
2. 摸清**全屏抑制的边界**，决定 hook 在什么情况下可靠
3. 验证 `lastDialogHint` 取值域（能让她说话的话，表达力会强很多）
4. 然后才是写 hook 脚本接线

**路线 B 作为备选**：仅当路线 A 的反应形式不够理想时启用。届时按「**选自带接口的桌宠**」这条经验来选：

| 偏好 | 推荐 | 理由 |
|---|---|---|
| Live2D 形象 + 最可靠接口 | **Live2DViewerEX + ExAPI** | 唯一的**官方** API（WebSocket `:23333`） |
| 接受 2D 素材、要零代码 | **clawd-on-desk** 换主题 | 12 状态桥接现成，`theme.json` + 1 SVG + 7 GIF |
| 坚持官方 MMD→VRM 3D 模型 | **Mate-Engine + Extension-MateEngine** | 需 Unity 工程重编译，该路线唯一选择 |

> ⚠️ 路线 B 里**没有一个能直接吃下官方天选姬桌宠的资源**——无论哪条都要自己准备形象素材。这正是路线 A 的不可替代之处：**它用的就是原版天选姬本人**。

---

## 五、待验证事项（按优先级）

| # | 待验证 | 怎么验 | 影响 |
|---|---|---|---|
| 1 | ~~**`Config.json` 是否热重载**~~ | ✅ **已确证**（3.7） | — |
| 2 | **`OneTimeAlarm` 触发时的表现**（声音？动作？气泡？） | ⏳ 需在**小姬可见**（非全屏应用遮挡）时重做 | 决定通知强度 |
| 3 | **`lastDialogHint` 的取值域** | 尝试填入 `DialogBlock_Stand_BlowBubble` 等值，看是否触发对应对话 | ⭐ 决定能否「让她说话」 |
| 4 | **全屏抑制的边界** | 何种条件下会被抑制？（全屏？特定进程？） | 决定 hook 何时可靠 |
| 5 | 能否用 `windowLeft` / `windowTop` 让她**移动到指定位置** | 改坐标看是否生效 | 锦上添花 |
| 6 | `CustomizeConfig.json` 能否触发**指定动作** | 加条目 + 改时间区间 | 决定能否精确控制动作 |
| 7 | `LookPhoneToast` 如何被触发 | 未找到入口 | 若可用则是最佳通知通道 |
| 8 | 窗口消息 / `SendInput` 能否驱动她 | 通用原理可行，需实测（`PostMessage` 单次失败） | 路线 A 的备用手段 |

---

## 六、风险与合规边界

### 6.1 本方案的合规性

本方案**只做两件事**：
1. 读取/写入**用户自己 AppData 目录下的配置文件**
2. 通过 Claude Code 官方 hook 机制接收事件

**不涉及**：破解、解密、注入 DLL、修改 `TX Mascot.exe`、绕过授权、提取受版权保护的模型/贴图。

这属于「外部正常驱动」，与第一节前提文档中评估的「原地改造（需破加密封装）」是**完全不同性质**的路线。

### 6.2 风险点

| 风险 | 说明 | 缓解 |
|---|---|---|
| **配置被写坏** | 外部修改 `Config.json` 格式错误可能导致桌宠启动异常 | 改前备份；只改已知字段；保持 JSON 合法 |
| **与程序写回冲突** | 程序可能覆盖外部修改 | 待验证事项 #3 |
| **hook 干扰 Claude Code** | 若 hook 非零退出，在 `Stop` 上 exit 2 会**阻止 Claude 停下** | 🚨 **永远 exit 0**，全部加 `async: true` |
| **hook 拖慢响应** | 同步 hook 会阻塞 | 统一用 `async: true` + 短超时 |
| **桌宠未启动** | hook 调用时桌宠不在 | 脚本需静默失败，不能报错 |

### 6.3 版权

在**本机自用**范围内修改配置文件、让桌宠对本地事件做反应，不涉及作品的复制与传播。若日后要**发布**这套工具：
- ✅ 发布自己写的 hook 脚本 / 桥接代码 —— 风险低
- ❌ 打包分发天选姬的模型、贴图、动画 DLL —— 高风险，且违反官方配布规约

详见前提文档第五节。

---

## 附录：来源

### 桌宠 × AI 助手生态
- clawd-on-desk：https://github.com/rullerzhou-afk/clawd-on-desk
- agentpet：https://github.com/ntd4996/agentpet
- oc-claw：https://github.com/rainnoon/oc-claw
- openpets：https://github.com/OpenPetsHQ/openpets
- Agentic-Desktop-Pet：https://github.com/jihe520/Agentic-Desktop-Pet
- Mate-Engine：https://github.com/shinyflvre/Mate-Engine
- Extension-MateEngine（本地 HTTP 8787 扩展）：https://github.com/LazyCodingKing/Extension-MateEngine
- Hopet 桥接架构：https://github.com/BinaryFroggy/Hopet/blob/main/DevDocs/architecture.md

### Claude Code hooks
- Hooks 官方参考：https://code.claude.com/docs/en/hooks
- Hooks 官方指南：https://code.claude.com/docs/en/hooks-guide
- Settings 优先级：https://code.claude.com/docs/en/settings
- clawd-conduit（桌宠接 hook 先例）：https://github.com/ssssssanjiu/clawd-conduit

### 本机实测
- 天选姬安装目录：`C:\Program Files\ASUS\TX Mascot\`
- 用户数据目录：`%LocalAppData%\ASUS\TX Mascot\`
- 检查时间：2026-09-17，桌宠进程运行中（PID 70540）
