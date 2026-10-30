import { mkdirSync, writeFileSync } from "node:fs";
import net from "node:net";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { saveUiFailure } from "./diagnostics.mjs";
import { browserArguments, systemMotion } from "./environment.mjs";

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
  mochaOpts: { timeout: 90000 },
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
          WEBVIEW2_USER_DATA_FOLDER: join(runDir, "webview"),
          WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: browserArguments,
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
    cases.push({ name: test.title, passed: result.passed, error: result.error?.message ?? null });
    if (!result.passed) {
      try {
        await saveUiFailure(cases.length, test.title, result.error?.message ?? "test-failed");
      } catch {
        console.error("[diagnostic] Failure report could not be written");
      }
      try {
        await browser.saveScreenshot(join(runDir, `failure-${cases.length}.png`));
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
