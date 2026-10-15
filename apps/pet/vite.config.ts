import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri 官方基线 + 双入口（架构设计 §8.2）：pet 首屏只加载 Pixi 不加载表单，settings 反之。
export default defineConfig({
  plugins: [
    react(),
    {
      name: "desktop-test-bootstrap",
      resolveId(id) {
        if (id === "virtual:desktop-test") return "\0desktop-test";
      },
      load(id) {
        if (id !== "\0desktop-test") return;
        if (process.env.VITE_E2E !== "1") return "export {};";
        const entry = fileURLToPath(
          new URL(
            "../../tests/desktop/node_modules/@wdio/tauri-plugin/dist-js/index.js",
            import.meta.url,
          ),
        );
        return `import ${JSON.stringify(entry.replaceAll("\\", "/"))};`;
      },
    },
  ],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    outDir: process.env.VITE_E2E === "1" ? "dist-e2e" : "dist",
    target: "chrome105",
    // Vite 8 = Rolldown/Oxc（不再捆绑 esbuild）；布尔值走默认压缩器
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
    rollupOptions: {
      input: {
        pet: "index.html",
        settings: "settings.html",
      },
    },
  },
});
