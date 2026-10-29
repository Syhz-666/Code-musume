# 内置角色 · 居家睡衣

`musume.json`、`spritesheet.png`、`thinking.png` 与 `working.png` 是正式运行时素材，不依赖被 Git 忽略的 `artifacts/`。
Rust 通过 `include_str!` / `include_bytes!` 打包，浏览器预览使用同一份清单和 PNG。

- 基础图集：3072 × 3328，16 列 × 16 行；思考图集：3072 × 2912，16 列 × 14 行。单格保持 192 × 208，透明 RGBA。
- 待机：243 帧，24fps，10.125 秒一轮，按行跨列连续存放在编号 0–242 的格子中。
- 思考分为进入 60 帧／48fps、持续循环 120 帧／24fps、退出 30 帧／60fps；分别占思考图集编号 0–59、60–179、180–209。持续期间只循环中段，离开时播放放手动作；进入中途被打断则沿当前抬手过程返回。
- 工作图集为3072×2080，进入36帧／48fps、持续88帧／24fps、退出30帧／60fps，编号分别0–35、36–123、124–153。退出反向复用真实抬手动作；工作与思考通过共同站姿衔接，支持进入中断。
- 其余五态保持原有单帧美术，仍放在基础图集编号245–249；原思考与工作静帧格243–244保留但不再由状态引用。桌宠显示尺寸仍为144×156。
- `row` 与 `column` 是动作起始格；之后按从左到右、从上到下读取 `frames` 个格子。旧包缺少 `column` 时按 0 处理。
- `Clip.image` 可选，缺省使用形态的 `image`；有值时使用该页。所有页采用同一帧格与列数；旧单页角色包保持兼容。
- “减少动画”直接显示对应持续段首帧，思考仍是手靠下巴、工作仍是双手在腰前；真实状态和手动预览使用相同播放规则。
- `default`、`claude`、`codex` 暂时共用这套睡衣，保留原有形态 ID 和映射兼容性。
- 不添加未经设计的换装、精灵或点击动作；现有团子绘制逻辑保留作资源加载失败时的回退。

## 制作记录

角色来源：项目确认的白色浅蓝长发、粉白睡衣角色，由内置 imagegen 生成。
待机动画：WorkBuddy 生成，经本地逐帧抠图和黑边修正后由用户确认可替换。确认的透明源帧保存在
`artifacts/design/preprocessed/frames/workbuddy-idle-cutout-v2/`。
思考动画来源为 WorkBuddy 2026-10-29 生成并经用户认可的透明候选 `artifacts/design/preprocessed/frames/workbuddy-thinking-cutout-v3/`。
工作动画来源为同日用户要求调整失败方法后生成的5秒单首帧片段。当前透明源为`artifacts/design/preprocessed/frames/workbuddy-working-cutout-v4/`，运行时打包在`runtime-working-v2/`。v4从原始抠图RGB重新解算绿幕污染，并分别参考已确认原图的暖白裤装、粉色裤脚和脚部肤色，修复v3去绿时产生的灰斑与灰黄脚部；124帧的透明度、上半身、坐标和时序与v3完全一致。v3与旧打包均保留作对照。原片实际1344×768（7:4），按比例补边处理而不拉伸。前两条10秒双帧失败记录完整保留。
本轮运行时打包记录和亮暗底预览保存在 `artifacts/design/preprocessed/frames/runtime-thinking-v1/`；旧 `runtime-idle-v1/` 保留作可复现输入。
所有帧使用恒定缩放和脚底锚点，不逐帧重新缩放或居中。原视频接缝经预乘 alpha 混合处理：待机尾部回到共同站姿，思考持续段回到同一思考姿势，进入首帧与退出末帧共用待机首帧。运行时在姿势交接时短暂混合，并支持快速打断。
绿幕定姿原图：`artifacts/design/preprocessed/keyframes/state-design-v1/`。
当前透明抠图、对齐单帧、亮暗底预览、校验数据：
`artifacts/design/preprocessed/frames/state-cutouts-v2/`。
v2 使用 `tools/chroma_key.py` 直接处理原始绿幕图，不再次调用图像模型。
原始画布和人物坐标保持不变；不透明主体像素的 RGB 逐点保持；半透明边缘按邻近原图前景色去绿。
此前由图像模型提取的 v1 及其提示词仍留在制作目录中，仅作历史记录。

未将新插画标为旧程序化样例的 MIT 授权，清单 `license` 保持 `null`。

重新打包（需要原始绿幕图；依赖版本见 `tools/requirements-artwork.txt`）：

```powershell
python tools/working_color.py
python tools/build-working-atlas.py --install
python -m unittest discover -s tools -p test_chroma_key.py -v
python -m unittest discover -s tools -p test_working_color.py -v
```

正式图集已放入本目录，可随源码纳入版本控制；普通构建不需要运行美术脚本或安装这些 Python 依赖。
旧思考打包工具在工作页已安装时会拒绝覆盖整份清单，避免误删新状态。
普通构建直接打包此目录中的 PNG 和清单；不依赖制作目录、视频模型或视频播放器。
