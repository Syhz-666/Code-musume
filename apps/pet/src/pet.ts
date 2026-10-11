import { PROTOCOL_VERSION } from "@code-musume/protocol";
import { invoke } from "@tauri-apps/api/core";
import { Application, Graphics } from "pixi.js";

/**
 * 桌宠窗口入口（Phase 1 骨架）：
 * - 透明 Pixi 画布 + 占位角色（会呼吸的圆——Phase 3 替换为 8 态图形配方）
 * - 首帧渲染完成后 invoke("pet_ready") 显示窗口（消灭白闪的最后一步）
 */
async function main(): Promise<void> {
  const host = document.getElementById("app");
  if (!host) throw new Error("#app not found");

  const app = new Application();
  await app.init({
    backgroundAlpha: 0,
    resizeTo: window,
    antialias: true,
    resolution: window.devicePixelRatio || 1,
    autoDensity: true,
  });
  host.appendChild(app.canvas);

  // 占位角色：圆 + 呼吸缩放
  const blob = new Graphics().circle(0, 0, 48).fill(0x8ecae6).stroke({ width: 3, color: 0x219ebc });
  blob.x = app.screen.width / 2;
  blob.y = app.screen.height / 2;
  app.stage.addChild(blob);

  let t = 0;
  app.ticker.add((ticker) => {
    t += ticker.deltaTime;
    blob.scale.set(1 + Math.sin(t / 40) * 0.06);
  });

  // 等两帧（确保 WebView 已实际呈现一帧）后再显示窗口
  await new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
  );
  try {
    await invoke("pet_ready");
  } catch (err) {
    // 直接在浏览器里打开 vite dev 时没有 Tauri 环境——忽略即可
    console.warn("[pet] pet_ready skipped (no Tauri host)", err);
  }
  console.info(`[pet] ready (protocol v${PROTOCOL_VERSION})`);
}

main().catch((err) => {
  console.error("[pet] init failed", err);
});
