# 固定桌面截图字体

`code-musume-test.woff2` 是 Noto Sans SC 的子集，改名为 Code Musume Test。
只在 `VITE_E2E=1` 的独立测试构建中加载，正常构建不包含该资源。
本地 Windows 与 CI 的系统字体、中文字体回退可能不同，因此截图不依赖系统字体。
原始授权见 [OFL.txt](OFL.txt)，固定上游版本、输入及输出 SHA-256 和字符集见 [source.json](source.json)。

重新生成需要 Python `fonttools[woff]`。从 `source.json` 的固定 URL 下载 TTF 后执行：

```powershell
python tools/build-test-font.py <下载的TTF路径>
pnpm exec biome format --write tests/desktop/fonts/source.json
```

生成器检查原文件 SHA-256，保留当前项目代码和测试使用到的字符以及 ASCII。
新增界面文字需要同步更新子集并审阅新的截图。界面测试等待字体加载完成，截图记录字体、语言、WebView 版本、DPR 和窗口大小。
像素阈值仍为 `0.1`，差异面积上限仍为 `0.5%`；更换字体产生的基准只能在独立审阅后更新。
