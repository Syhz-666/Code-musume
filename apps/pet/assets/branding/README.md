# 代码娘头像与应用图标

`avatar.png` 是用户于 2026-10-29 确认的正面头像：头部端正、双眼水平，保留白色浅蓝长发、蓝灰眼与粉白睡衣领口。512×512 RGBA，背景透明。

在项目根目录运行：

```powershell
python tools/build-app-icons.py --install
```

脚本用 Pillow 缩放此母版，生成 `apps/pet/src-tauri/icons/` 中的通用 PNG、九种尺寸的 Windows ICO，以及专用的 32×32 托盘图标。托盘由宿主显式加载 `tray-icon.png`；Windows 程序与后续 NSIS 构建使用 `icon.ico`。

生成记录与原图位于 `artifacts/design/icon-candidates/tray-avatar-v2-upright/`，首次替换前的图标备份位于 `artifacts/design/backups/icons-before-avatar-2026-10-29/`。这两个目录属于本机设计归档；重新打包只依赖本目录中的母版。
