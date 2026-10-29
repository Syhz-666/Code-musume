import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";
import { localRequest } from "../../tools/capture-live-hooks.mjs";

export const root = fileURLToPath(new URL("../..", import.meta.url));
export const runDir = process.env.CODE_MUSUME_UI_RUN_DIR;
export const visuals = [];
const renderedLatencies = [];
// WDIO 1.5.0 caches its async callback by window label. Use the standard sync
// endpoint (which awaits promises) so commands also work after window recreation.
export async function nativeInvoke(command, args = {}) {
  return browser.execute(
    async (name, values) => window.__TAURI_INTERNALS__.invoke(name, values),
    command,
    args,
  );
}
function isNavigationTimeout(error) {
  return /Script execution timed out/i.test(String(error));
}
export async function selectWindow(label, { prepare = true } = {}) {
  await browser.waitUntil(async () => (await browser.getWindowHandles()).includes(label));
  await browser.switchToWindow(label);
  await browser.waitUntil(
    async () => {
      try {
        return await browser.execute(
          (label) =>
            label === "pet"
              ? !!document.querySelector("#app")?.dataset.instance
              : !!document.querySelector('[data-testid="settings"]'),
          label,
        );
      } catch (error) {
        if (isNavigationTimeout(error)) return false;
        throw error;
      }
    },
    { timeoutMsg: `Window ${label} is not initialized` },
  );
  if (!prepare) return;
  await browser.waitUntil(
    async () => {
      try {
        return await browser.execute(async () => {
          const faces = await document.fonts.load('12px "Code Musume Test"');
          await document.fonts.ready;
          return faces.length > 0 && document.fonts.check('12px "Code Musume Test"');
        });
      } catch (error) {
        if (isNavigationTimeout(error)) return false;
        throw error;
      }
    },
    { timeoutMsg: "Bundled screenshot font did not load" },
  );
  const isPet = label === "pet";
  await browser.waitUntil(
    async () => {
      try {
        await nativeInvoke("plugin:window|set_size", {
          label,
          value: { Physical: { width: isPet ? 160 : 850, height: isPet ? 210 : 760 } },
        });
        return true;
      } catch (error) {
        if (isNavigationTimeout(error)) return false;
        throw error;
      }
    },
    { timeoutMsg: `Window ${label} did not reach its physical test size` },
  );
  await browser.waitUntil(
    async () => {
      try {
        return await browser.execute(() => {
          const style = document.createElement("style");
          style.textContent =
            "*,*::before,*::after{transition:none!important;animation:none!important;caret-color:transparent!important}";
          document.head.append(style);
          return true;
        });
      } catch (error) {
        if (isNavigationTimeout(error)) return false;
        throw error;
      }
    },
    { timeoutMsg: `Window ${label} did not finish navigation` },
  );
}
export async function health() {
  const path = join(runDir, "runtime/runtime.dev.json");
  await browser.waitUntil(() => existsSync(path));
  const runtime = JSON.parse(readFileSync(path, "utf8"));
  const response = await localRequest(runtime, "GET", "/v2/health", undefined, 2000);
  assert.equal(response.status, 200);
  return JSON.parse(response.body);
}
export async function hook(agent, event, extra = {}, session = "ui-session", turn = "ui-turn") {
  const payload = {
    hook_event_name: event,
    session_id: session,
    [agent === "claude-code" ? "prompt_id" : "turn_id"]: turn,
    ...extra,
  };
  await new Promise((ok, reject) => {
    const child = spawn(
      join(root, "target/debug/code-musume-hook.exe"),
      [
        "--agent",
        agent,
        "--event",
        event,
        "--runtime-file",
        join(runDir, "runtime/runtime.dev.json"),
      ],
      { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] },
    );
    let output = "";
    child.stdout.on("data", (b) => {
      output += b;
    });
    child.stderr.on("data", (b) => {
      output += b;
    });
    child.on("error", reject);
    child.on("exit", (code) => {
      try {
        assert.equal(code, 0);
        assert.equal(output, "");
        ok();
      } catch (e) {
        reject(e);
      }
    });
    child.stdin.on("error", reject);
    child.stdin.end(JSON.stringify(payload));
  });
}
export async function pet(expected, { latency = false, prepare = true } = {}) {
  await selectWindow("pet", { prepare });
  let lastActual;
  await browser
    .waitUntil(
      async () => {
        let actual;
        try {
          actual = await browser.execute(() => ({ ...document.querySelector("#app")?.dataset }));
        } catch (error) {
          if (isNavigationTimeout(error)) return false;
          throw error;
        }
        lastActual = actual;
        return (
          Object.entries(expected).every(([k, v]) => actual[k] === String(v)) &&
          actual.revision === actual.renderedRevision
        );
      },
      { timeoutMsg: `Pet did not render ${JSON.stringify(expected)}` },
    )
    .catch((error) => {
      error.message += `; last view: ${JSON.stringify(lastActual)}`;
      throw error;
    });
  if (latency) {
    const result = await health();
    assert.equal(result.renderReceipt?.instanceId, result.projection.instanceId);
    assert.equal(result.renderReceipt?.revision, result.projection.revision);
    assert(
      result.renderReceipt.latencyMs <= 1000,
      `Rendered latency ${result.renderReceipt.latencyMs}ms exceeds 1000ms`,
    );
    renderedLatencies.push({
      state: result.projection.state,
      agent: result.projection.dominantAgent,
      ...result.renderReceipt,
    });
    writeFileSync(
      join(runDir, "render-latencies.json"),
      JSON.stringify(renderedLatencies, null, 2),
    );
    writeFileSync(join(runDir, "latest-render-receipt.json"), JSON.stringify(result, null, 2));
  }
}
export async function tab(name) {
  await selectWindow("settings");
  const id = {
    工作联动: "connect",
    角色与形态: "character",
    使用偏好: "preferences",
    诊断: "diagnostics",
  }[name];
  await (await browser.$(`[data-testid="tab-${id}"]`)).waitForDisplayed();
  await (await browser.$(`[data-testid="tab-${id}"]`)).click();
}
export async function manualPet(state, realState, { prepare = true } = {}) {
  await selectWindow("pet", { prepare });
  await browser.waitUntil(
    async () => {
      const actual = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
      return (
        actual.animationMode === "manual" &&
        actual.displayState === state &&
        actual.state === realState
      );
    },
    { timeoutMsg: `Pet did not preview ${state} while real state is ${realState}` },
  );
  assert.equal(await (await browser.$(".agent-label")).getText(), "手动预览");
}
export async function choose(selector, value) {
  await browser.execute(
    (selector, value) => {
      const control = document.querySelector(selector);
      if (control?.tagName !== "SELECT") throw new Error(`Missing select: ${selector}`);
      control.value = value;
      control.dispatchEvent(new Event("input", { bubbles: true }));
      control.dispatchEvent(new Event("change", { bubbles: true }));
    },
    selector,
    value,
  );
  assert.equal(await (await browser.$(selector)).getValue(), value);
}
export async function screenshot(name) {
  await browser.execute(() => {
    document.activeElement?.blur();
    window.scrollTo(0, 0);
  });
  const environment = await browser.execute(() => ({
    dpr: devicePixelRatio,
    width: innerWidth,
    height: innerHeight,
    userAgent: navigator.userAgent,
    font: getComputedStyle(document.body).fontFamily,
    language: navigator.language,
  }));
  assert.equal(environment.dpr, 1, "Visual baseline requires controlled DPR=1");
  const bytes = Buffer.from(await browser.takeScreenshot(), "base64");
  const image = PNG.sync.read(bytes);
  const baseline = join(root, "tests/desktop/baselines", `${name}.png`);
  const reviewPath = join(root, "tests/desktop/baselines/review.json");
  const approval = existsSync(reviewPath)
    ? JSON.parse(readFileSync(reviewPath, "utf8")).images?.[name]
    : null;
  const actual = join(runDir, `${name}.png`);
  writeFileSync(actual, bytes);
  if (
    process.env.CODE_MUSUME_UI_REVIEW === "1" ||
    !existsSync(baseline) ||
    !approval?.reviewer ||
    !approval?.reviewedAt
  ) {
    visuals.push({ name, status: "pending-review", actual, environment });
  } else {
    assert.equal(
      createHash("sha256").update(readFileSync(baseline)).digest("hex"),
      approval.sha256,
      `${name}: baseline changed after review`,
    );
    const expected = PNG.sync.read(readFileSync(baseline));
    assert.equal(image.width, expected.width);
    assert.equal(image.height, expected.height);
    const diff = new PNG({ width: image.width, height: image.height });
    const pixels = pixelmatch(expected.data, image.data, diff.data, image.width, image.height, {
      threshold: 0.1,
    });
    const ratio = pixels / (image.width * image.height);
    writeFileSync(join(runDir, `${name}.diff.png`), PNG.sync.write(diff));
    visuals.push({ name, status: ratio <= 0.005 ? "passed" : "failed", ratio, environment });
    writeFileSync(join(runDir, "visual-results.json"), JSON.stringify(visuals, null, 2));
    // Keep exercising the remaining states. The suite's final hook fails on any
    // visual mismatch, without leaving a permission/preview state mid-scenario.
  }
  writeFileSync(join(runDir, "visual-results.json"), JSON.stringify(visuals, null, 2));
}
export function makeCharacter() {
  const directory = join(runDir, "character-input");
  mkdirSync(directory, { recursive: true });
  const image = new PNG({ width: 32, height: 32 });
  for (let y = 0; y < 32; y++)
    for (let x = 0; x < 32; x++) {
      const i = (y * 32 + x) * 4;
      image.data[i] = 180;
      image.data[i + 1] = 140;
      image.data[i + 2] = 230;
      image.data[i + 3] = (x - 16) ** 2 + (y - 16) ** 2 < 180 ? 255 : 0;
    }
  writeFileSync(join(directory, "pet.png"), PNG.sync.write(image));
  const clips = Object.fromEntries(
    [
      "idle",
      "thinking",
      "working",
      "waiting-permission",
      "done",
      "notification",
      "error",
      "sleeping",
    ].map((s) => [s, { row: 0, frames: 1, fps: 1, looping: true }]),
  );
  writeFileSync(
    join(directory, "musume.json"),
    JSON.stringify({
      schemaVersion: 1,
      id: "fixture",
      name: "自动验收示例",
      defaultForm: "default",
      agentForms: { "claude-code": "claude", codex: "codex" },
      license: "MIT — generated test artwork",
      forms: ["default", "claude", "codex"].map((id) => ({
        id,
        image: "pet.png",
        cellWidth: 32,
        cellHeight: 32,
        columns: 1,
        clips,
        lookRows: false,
      })),
    }),
  );
  writeFileSync(join(directory, "LICENSE"), "MIT — generated test artwork\n");
  return directory;
}
