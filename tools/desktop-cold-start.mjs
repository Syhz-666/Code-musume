import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import http from "node:http";
import net from "node:net";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { localRequest } from "./capture-live-hooks.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const directory = process.env.CODE_MUSUME_UI_RUN_DIR;
if (!directory) throw new Error("An isolated UI run directory is required");
const runtimePath = join(directory, "runtime/runtime.dev.json");
const reports = [];
const pause = (ms) => new Promise((ok) => setTimeout(ok, ms));
async function request(port, method, path, value) {
  return new Promise((ok, reject) => {
    const body = value ? JSON.stringify(value) : undefined;
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port,
        method,
        path,
        agent: false,
        headers: { "content-type": "application/json" },
      },
      (res) => {
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => {
          try {
            const data = JSON.parse(Buffer.concat(chunks).toString("utf8"));
            if (res.statusCode >= 400)
              reject(new Error(data.value?.message ?? String(res.statusCode)));
            else ok(data.value);
          } catch (e) {
            reject(e);
          }
        });
      },
    );
    req.setTimeout(2000, () => req.destroy(new Error("test-driver-timeout")));
    req.on("error", reject);
    req.end(body);
  });
}
async function check(name) {
  const probe = net.createServer();
  await new Promise((ok) => probe.listen(0, "127.0.0.1", ok));
  const port = probe.address().port;
  await new Promise((ok) => probe.close(ok));
  const app = spawn(join(root, "target/e2e/debug/pet.exe"), [], {
    windowsHide: true,
    stdio: "ignore",
    env: {
      ...process.env,
      CODE_MUSUME_DATA_DIR: join(directory, "runtime"),
      CODE_MUSUME_CONFIG_DIR: join(directory, "config"),
      WEBVIEW2_USER_DATA_FOLDER: join(directory, "webview"),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: "--force-device-scale-factor=1",
      TAURI_WEBDRIVER_PORT: String(port),
    },
  });
  const exited = new Promise((ok) => app.once("exit", (code) => ok({ code })));
  try {
    const until = Date.now() + 15000;
    let runtime;
    while (Date.now() < until) {
      try {
        const candidate = JSON.parse(readFileSync(runtimePath, "utf8"));
        if (candidate.pid === app.pid) {
          runtime = candidate;
          break;
        }
      } catch {
        /* No current publication yet. */
      }
      if (app.exitCode !== null) throw new Error("cold-start application exited early");
      await pause(50);
    }
    assert(runtime, "cold-start runtime was not published");
    let session;
    while (Date.now() < until) {
      try {
        session = await request(port, "POST", "/session", { capabilities: { alwaysMatch: {} } });
        break;
      } catch {
        await pause(50);
      }
    }
    assert(session?.sessionId, "cold-start WebView driver unavailable");
    const sessionId = session.sessionId;
    let frame;
    while (Date.now() < until) {
      frame = await request(port, "POST", `/session/${sessionId}/execute/sync`, {
        script: "return Object.assign({}, document.querySelector('#app')?.dataset)",
        args: [],
      });
      if (frame?.state === "idle" && frame.revision === frame.renderedRevision) break;
      await pause(50);
    }
    assert.equal(frame.state, "idle");
    assert.equal(frame.agent, "");
    assert.equal(frame.pending, "0");
    assert.equal(frame.character, "builtin");
    assert.equal(frame.renderer, "dom");
    const health = JSON.parse((await localRequest(runtime, "GET", "/v2/health")).body);
    assert.equal(health.projection.activeSessions, 0);
    const config = JSON.parse(readFileSync(join(directory, "config/config.json"), "utf8"));
    assert.equal(config.renderer, "dom");
    assert.equal(config.reducedMotion, true);
    const exitVia = name.startsWith("damaged-") ? "settings" : "pet";
    if (exitVia === "settings") {
      await request(port, "POST", `/session/${sessionId}/execute/sync`, {
        script: "window.__TAURI_INTERNALS__.invoke('show_settings'); return true",
        args: [],
      });
      let settingsReady = false;
      while (Date.now() < until) {
        const handles = await request(port, "GET", `/session/${sessionId}/window/handles`);
        if (handles.includes("settings")) {
          settingsReady = true;
          break;
        }
        await pause(50);
      }
      assert(settingsReady, "settings window did not open for exit check");
      await request(port, "POST", `/session/${sessionId}/window`, { handle: "settings" });
    }
    const selector = exitVia === "pet" ? ".exit-button" : '[data-testid="exit-app"]';
    let exitReady = false;
    while (Date.now() < until) {
      exitReady = await request(port, "POST", `/session/${sessionId}/execute/sync`, {
        script:
          "const b = document.querySelector(arguments[0]); return !!b && !b.disabled && b.getBoundingClientRect().height > 0",
        args: [selector],
      });
      if (exitReady) break;
      await pause(50);
    }
    assert(exitReady, `${exitVia} exit button is unavailable`);
    await request(port, "POST", `/session/${sessionId}/execute/sync`, {
      script:
        "const b = document.querySelector(arguments[0]); setTimeout(() => b.click(), 30); return true",
      args: [selector],
    });
    const exit = await Promise.race([exited, pause(5000).then(() => null)]);
    assert(exit, `${exitVia} exit button did not terminate the application`);
    assert.equal(exit.code, 0, "application did not exit cleanly");
    assert(!existsSync(runtimePath), "application exit left its runtime publication behind");
    reports.push({
      name,
      passed: true,
      frame,
      projection: health.projection,
      exitVia,
      exitCode: exit.code,
      runtimeRemoved: true,
    });
  } catch (error) {
    reports.push({ name, passed: false, error: error.message });
    throw error;
  } finally {
    if (app.exitCode === null) app.kill();
    writeFileSync(join(directory, "cold-start-results.json"), JSON.stringify(reports, null, 2));
  }
}
try {
  await check("whole-application-restart-preserves-settings-and-clears-work");
  const characterRoot = join(directory, "config/characters");
  const imported = readdirSync(characterRoot).find((id) => id.startsWith("import-"));
  assert(imported, "the UI import scenario did not create a test character");
  const manifest = JSON.parse(readFileSync(join(characterRoot, imported, "musume.json"), "utf8"));
  const configPath = join(directory, "config/config.json");
  const config = JSON.parse(readFileSync(configPath, "utf8"));
  config.characterId = imported;
  writeFileSync(configPath, JSON.stringify(config));
  writeFileSync(join(characterRoot, imported, manifest.forms[0].image), "damaged test image");
  await check("damaged-character-falls-back-to-builtin-with-valid-preferences");
  console.log("PASS: restart, damaged-character recovery and both visible exit buttons");
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
