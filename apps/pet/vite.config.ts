import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri 官方基线 + 双入口（架构设计 §8.2）：pet 首屏只加载 Pixi 不加载表单，settings 反之。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
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
