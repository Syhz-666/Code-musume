# 粉白睡衣待机母图草稿 v1

## 状态

待确认草稿，不是通过技术验收的最终母版。仅制作一张待机基准图，未生成新的动作关键帧、视频或运行时素材，未修改程序。

## 参考与生成方式

- 生成方式：内置 image_gen，以本地已确认的角色图作为实际图片输入；未使用 CLI/API 后备模式。
- 主参考：`artifacts/design/code-musume-homewear-v1.png` 的最左侧正面全身形象，决定角色身份、睡衣和画风。
- 辅助参考：`artifacts/design/code-musume-base-concept-v3-chibi.png`，仅用于角色身份与头身比例，不采用其背心短裤。
- 不使用之前带噪点的方形 K0/K1 作为参考。
- 第一轮从上述两张设定图制作单人绿幕母图；第二轮以第一轮结果为编辑目标，仅要求技术性缩小人物、保持中心并统一背景。
- 项目内图片：`art/code-musume/keyframes/idle/homewear-idle-master-draft-v1.png`
- 两轮原始输出分别为：
  - `C:/Users/admin/.codex/generated_images/01a14448-11a1-73e1-80f2-5774c7d5c2b9/exec-dfd95f7e-86ef-4871-a5fe-96289300aa44.png`
  - `C:/Users/admin/.codex/generated_images/01a14448-11a1-73e1-80f2-5774c7d5c2b9/exec-96c83b5d-ef0d-4945-94e6-0b02b37b7cb1.png`
- 第二轮输出原样复制到项目，没有通过程序修改、去噪、抠像或重采样。

## 检查结果

- 实际尺寸 1672 × 941；接近 16:9，但不是数学上严格的 16:9，也不是提示词指定的 2048 × 1152。
- 以每 3 像素采样、颜色阈值区分人物和绿幕，估计人物头发最高点 Y=19.13%，脚底 Y=85.76%，最左 X=40.91%，最右 X=58.67%。这是近似检查，不是精确 alpha 轮廓测量。
- 人物未裁切，安全区符合要求；位置接近母图目标。
- 背景 x=30..129、y=30..129 的 100 × 100 采样区存在 38 种颜色，严格 #00FF00 像素占比为 0%。背景仍有细微纹理，未通过纯色验收。
- 已停止继续生成。角色外观需用户确认；严格画幅、背景颜色和噪点问题仍需解决，不将此草稿称为成品母版。

## 第一轮提示词（完整）

```text
Use case: identity-preserve
Asset type: single desktop companion idle master reference, not an animation or a character sheet.

INPUTS AND AUTHORITY:
Image 1 is the approved CODE-MUSUME HOMEWEAR CONCEPT 01 sheet: this is the authoritative character and outfit reference. Extract and faithfully preserve the LEFTMOST full-body FRONT VIEW character. Image 2 is the approved CHIBI CONCEPT 03 identity sheet: supporting reference for the SAME face, hair, proportions and original drawing style ONLY; do NOT copy its tank top/shorts.
This is a faithful reference-image edit and recomposition, NOT a new character design. Preserve the exact recognizable face, eye shapes and charcoal/pale-blue eye palette, bang layout, white hair with pale blue strands and tips, hair length and silhouette, tiny human ears, warm fair complexion, petite approximately three-head-tall anime chibi proportions. No animal ears, tail, ornaments, jewelry or new accessories.
Preserve Image 1's loose pale-pink long-sleeved pajama shirt with its white rounded collar piping, placket, four buttons, one chest pocket, white hem piping and cuff details; off-white loose pajama pants with pink cuffs; bare feet. Preserve the approved fine anime linework, palette and restrained clean shading. Do not redesign the clothing or make the body taller/slimmer/wider.

SCENE AND OUTPUT:
Exactly ONE full-body front-facing character on one WIDE 16:9 landscape canvas, target 2048 x 1152 pixels. No sheet, panels, alternate views, portraits, swatches, headings, captions or text.
Background: uniform digital RGB (0,255,0), hexadecimal #00FF00, over the entire empty canvas. It must look like a completely flat solid digital color fill, not a lit green backdrop. Absolutely no green gradient, grain, stippling, mottling, atmospheric shading, floor, horizon, contact shadow, cast shadow or objects. No green color spill onto the character.

POSE AND EXPRESSION:
Faithfully use the approved leftmost front-view standing pose: facing straight toward viewer, head upright, arms loosely relaxed beside body, hands visible, feet flat. Open eyes, a subtle warm CLOSED-MOUTH smile, relaxed and gently attentive. Do not bounce, gesture, inhale exaggeratedly or tilt the head. This image is the neutral static idle anchor.

PRECISE COMPOSITION:
Image coordinates use the top-left as (0%,0%). Horizontal character center X=50%. Highest point of the hair at Y approximately 20%; bottom of both soles on a common baseline Y approximately 85%. Thus the complete character silhouette occupies approximately 65% of canvas height, leaving 20% empty green space above and 15% below. Keep the character intentionally SMALL within the wide canvas; do not zoom in or fill the frame.
Preserve NATURAL APPROVED CHARACTER PROPORTIONS. Fit every point of the character within horizontal X=25% to X=75%; those are containment bounds, NOT instructions to stretch the character to span 50% of canvas width. The figure will naturally be considerably narrower. No body part cropped. At least 10% margin from every image edge.

CLEANLINESS:
Clean production-ready anime color shapes; preserve intentional shading/highlights but avoid added textured brushwork, speckle, dirty patches, film grain, dithering, glints/sparkles floating on skin or clothing, messy lines or fuzzy edges.
Change only the sheet layout into the specified single-figure composition, replace the background, and close the smiling mouth. Preserve the approved character's identity, anatomy, hair, outfit and overall art style.
```

## 第二轮提示词（完整）

```text
Use case: identity-preserve
Edit target: the provided single-character green-screen image.
Make ONLY a technical framing/background correction. Do NOT redesign or re-illustrate the character. Preserve her exact face, eyes, expression, hair, anatomy, pose, pajama details, colors and line art.

The character is currently too large. Uniformly REDUCE THE WHOLE CHARACTER TO 80% OF ITS CURRENT SIZE, keeping the same center point at canvas X=50%, Y=52.5%. Keep the wide 16:9 canvas dimensions unchanged. This means the top of her hair must move DOWN from about 12% to 20% of image height, and the soles must move UP from about 93% to 85%. Final figure height =65% of the canvas height. Leave very large empty green margins. Do not zoom the canvas or crop it after reducing her. Preserve the character aspect ratio; do not widen or elongate her.

Replace every empty-background area with one absolutely UNIFORM digital RGB(0,255,0), #00FF00. No grain, no mottling, no gradients, no shadows, no green spill. This is a flat digital chroma color fill, not a physical green backdrop.
Keep a single front-facing closed-mouth-smiling character, no props, text, additional views or borders.
```
