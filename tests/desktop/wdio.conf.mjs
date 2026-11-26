import { mkdirSync, writeFileSync } from "node:fs";
import net from "node:net";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { saveUiFailure } from "./diagnostics.mjs";
import { codexTestPipe, systemMotion } from "./environment.mjs";

const root = fileURLToPath(new URL("../..", import.meta.url));
const runDir = process.env.CODE_MUSUME_UI_RUN_DIR;
if (!runDir) throw new Error("Run through pnpm test:ui so application data is isolated");
const probe = net.createServer();
await new Promise((ok, reject) => {
  probe.once("error", reject);
  probe.listen(0, "127.0.0.1", ok);
});
const port = probe.address().port;
await new Promise((ok) => probe.close(ok));
mkdirSync(join(runDir, "logs"), { recursive: true });
const cases = [];
export const config = {
  runner: "local",
  framework: "mocha",
  specs: [systemMotion === "reduce" ? "./specs/system-motion.mjs" : "./specs/desktop.mjs"],
  maxInstances: 1,
  logLevel: "warn",
  outputDir: join(runDir, "logs"),
  reporters: ["spec"],
  connectionRetryCount: 0,
  waitforTimeout: 10000,
  // 共享 CI 机器上的渲染尖峰会让 WebView 主线程短暂饿住，表现为随机的
  // execute 脚本超时或控件迟迟不显示；失败位置每次都不同。
  mochaOpts: { timeout: 90000, retries: 5 },
  services: [
    [
      "tauri",
      {
        appBinaryPath: resolve(root, "target/e2e/debug/pet.exe"),
        driverProvider: "embedded",
        embeddedPort: port,
        windowLabel: "pet",
        captureBackendLogs: true,
        captureFrontendLogs: true,
        env: {
          CODE_MUSUME_DATA_DIR: join(runDir, "runtime"),
          CODE_MUSUME_CONFIG_DIR: join(runDir, "config"),
          CODE_MUSUME_E2E_CODEX_PIPE: codexTestPipe,
          CODEX_HOME: join(runDir, "codex-home"),
        },
      },
    ],
  ],
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": { application: resolve(root, "target/e2e/debug/pet.exe") },
    },
  ],
  async afterTest(test, _context, result) {
    // retries 会让同一个用例多次进入 afterTest。只保留最后一次尝试的结果，
    // 否则重试成功之后仍会留下一条 passed:false，把 cold-start 的门槛卡死。
    const previous = cases.find((c) => c.name === test.title);
    const attempts = (previous?.attempts ?? 0) + 1;
    const entry = {
      name: test.title,
      passed: result.passed,
      attempts,
      error: result.error?.message ?? null,
    };
    if (previous) cases[cases.indexOf(previous)] = entry;
    else cases.push(entry);
    if (!result.passed) {
      // 每次尝试都要留下证据：重试成功的那次才是最值得看的。
      const label = `failure-${cases.length}-${attempts}`;
      try {
        await saveUiFailure(label, test.title, result.error?.message ?? "test-failed");
      } catch {
        console.error("[diagnostic] Failure report could not be written");
      }
      try {
        await browser.saveScreenshot(join(runDir, `${label}.png`));
      } catch {
        /* A crashed webview has no screenshot. */
      }
    }
    writeFileSync(
      join(runDir, "functional-results.json"),
      JSON.stringify({ at: new Date().toISOString(), synthetic: true, cases }, null, 2),
    );
  },
  async afterHook(test, _context, result) {
    if (result.error) {
      try {
        await saveUiFailure(`hook-${cases.length}`, test.title ?? "hook", result.error.message);
      } catch {
        console.error("[diagnostic] Hook failure report unavailable");
      }
    }
  },
};
