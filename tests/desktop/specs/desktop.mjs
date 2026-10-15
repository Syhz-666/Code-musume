import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  choose,
  health,
  hook,
  makeCharacter,
  pet,
  runDir,
  screenshot,
  selectWindow,
  tab,
  visuals,
} from "../helpers.mjs";

const cc = "claude-code",
  cx = "codex";
describe("Actual desktop event chain and settings", () => {
  before(async () => {
    await selectWindow("pet");
    await pet({ state: "idle" });
    assert(await (await browser.$('[aria-label="退出代码娘"]')).isDisplayed());
    assert(await (await browser.$('[aria-label="退出代码娘"]')).isEnabled());
    await (await browser.$('[aria-label="打开设置"]')).click();
    await selectWindow("settings");
  });
  it("opens all four settings pages and checks their visible content", async () => {
    assert(await (await browser.$('[data-testid="exit-app"]')).isDisplayed());
    assert(await (await browser.$('[data-testid="exit-app"]')).isEnabled());
    for (const [name, heading, image] of [
      ["工作联动", "连接你的工作", "settings-connect"],
      ["角色与形态", "一个角色，不同面貌", "settings-character"],
      ["使用偏好", "让她适合你的桌面", "settings-preferences"],
      ["诊断", "检查工作联动", "settings-diagnostics"],
    ]) {
      await tab(name);
      assert.equal(await (await browser.$("h2")).getText(), heading);
      await screenshot(image);
    }
  });
  for (const renderer of ["pixi", "dom"]) {
    it(`${renderer}: renders eight states through the real CLI and native core`, async () => {
      await tab("使用偏好");
      await choose('[data-testid="renderer-select"]', renderer);
      await (await browser.$("button=保存使用偏好")).click();
      await pet({ renderer, state: "idle" });
      await screenshot(`${renderer}-idle`);
      await hook(cc, "UserPromptSubmit");
      await pet({ state: "thinking", agent: cc, form: "claude" }, { latency: true });
      await screenshot(`${renderer}-thinking`);
      await hook(cc, "PreToolUse", { tool_use_id: "one", tool_name: "Read" });
      await pet({ state: "working" }, { latency: true });
      await screenshot(`${renderer}-working`);
      await hook(cc, "PermissionRequest", { tool_use_id: "one" });
      await pet({ state: "waiting-permission", pending: 1 }, { latency: true });
      await screenshot(`${renderer}-waiting`);
      await hook(cc, "PostToolUse", { tool_use_id: "one" });
      await pet({ state: "thinking", pending: 0 });
      await hook(cc, "Notification", { notification_type: "idle_prompt" });
      await pet({ state: "notification" }, { latency: true });
      await screenshot(`${renderer}-notification`);
      await browser.tauri.execute(({ core }) => core.invoke("e2e_advance_clock", { ms: 4001 }));
      await hook(cc, "PostToolUseFailure", { tool_use_id: "failed" });
      await pet({ state: "error" }, { latency: true });
      await screenshot(`${renderer}-error`);
      await browser.tauri.execute(({ core }) => core.invoke("e2e_advance_clock", { ms: 30001 }));
      await hook(cc, "Stop");
      await pet({ state: "done" }, { latency: true });
      await screenshot(`${renderer}-done`);
      await browser.tauri.execute(({ core }) => core.invoke("e2e_advance_clock", { ms: 600001 }));
      await pet({ state: "sleeping" });
      await screenshot(`${renderer}-sleeping`);
      await browser.tauri.execute(({ core }) => core.invoke("reset_sessions"));
      await pet({ state: "idle" });
    });
  }
  it("keeps waiting isolated between sources and restores the other agent after done", async () => {
    await hook(cc, "PreToolUse", { tool_use_id: "a" });
    await hook(cx, "PreToolUse", { tool_use_id: "b" });
    await pet({ state: "working", agent: cc });
    assert.equal((await health()).projection.activeSessions, 2);
    await hook(cx, "PermissionRequest", { tool_use_id: "b" });
    await pet({ state: "waiting-permission", agent: cx, form: "codex", pending: 1 });
    await hook(cc, "Stop");
    await pet({ state: "waiting-permission", agent: cx, pending: 1 });
    await hook(cx, "Interrupt");
    await pet({ state: "done", agent: cc, pending: 0 });
    await browser.tauri.execute(({ core }) => core.invoke("reset_sessions"));
    await hook(cc, "PreToolUse", { tool_use_id: "a" });
    await hook(cx, "PreToolUse", { tool_use_id: "b" });
    await hook(cc, "Stop");
    await pet({ state: "done", agent: cc });
    await browser.tauri.execute(({ core }) => core.invoke("e2e_advance_clock", { ms: 3001 }));
    await pet({ state: "working", agent: cx, form: "codex" });
    await screenshot("both-restore-codex");
  });
  it("saves form settings through controls and preserves them across page reload", async () => {
    await tab("角色与形态");
    await choose('[data-testid="form-select"]', "claude");
    await (await browser.$("button=保存角色选择")).click();
    await pet({ agent: cx, form: "claude" });
    await selectWindow("settings");
    await browser.refresh();
    await tab("角色与形态");
    assert.equal(await (await browser.$$("select"))[1].getValue(), "claude");
    assert.equal(
      JSON.parse(readFileSync(join(runDir, "config/config.json"), "utf8")).formLock,
      "claude",
    );
    await choose('[data-testid="form-select"]', "");
    await (await browser.$("button=保存角色选择")).click();
    await pet({ form: "codex" });
  });
  it("imports real image data and shows an actionable error for invalid input", async () => {
    await tab("角色与形态");
    const directory = makeCharacter();
    await (await browser.$('[aria-label="角色来源路径"]')).setValue(directory);
    await (await browser.$("button=导入并使用")).click();
    await browser.waitUntil(async () =>
      (await browser.$('[role="status"]')).getText().then((t) => t.includes("已导入")),
    );
    const imported = (await health()).projection.characterId;
    assert(imported.startsWith("import-"));
    assert(existsSync(join(runDir, "config/characters", imported, "LICENSE")));
    await pet({ character: imported, form: "codex" });
    await screenshot("imported-character");
    await tab("角色与形态");
    await (await browser.$('[aria-label="角色来源路径"]')).setValue(join(directory, "missing.png"));
    await (await browser.$("button=导入并使用")).click();
    await (await browser.$('[role="alert"]')).waitForDisplayed();
    assert.equal((await health()).projection.characterId, imported);
    await choose('[data-testid="character-select"]', "builtin");
    await (await browser.$("button=保存角色选择")).click();
    await pet({ character: "builtin" });
  });
  it("recovers the latest snapshot after hiding, refreshing and recreating the pet window", async () => {
    await tab("使用偏好");
    const hidden = await browser.$('[data-testid="preference-userHidden"]');
    await hidden.click();
    await (await browser.$("button=保存使用偏好")).click();
    await hook(cx, "PermissionRequest", { tool_use_id: "hidden" });
    await hidden.click();
    await (await browser.$("button=保存使用偏好")).click();
    await pet({ state: "waiting-permission", pending: 1 });
    await browser.refresh();
    await pet({ state: "waiting-permission", pending: 1 });
    await selectWindow("settings");
    await browser.tauri.execute(({ core }) => core.invoke("e2e_recreate_pet"));
    // WDIO 1.5.0 retains its async message handler by label across destruction.
    // The standard synchronous WebDriver channel reads the newly created WebView directly.
    await pet({ state: "waiting-permission", pending: 1 }, { prepare: false });
    await tab("诊断");
    await (await browser.$("button=刷新事件记录")).click();
    await browser.waitUntil(async () =>
      (await (await browser.$("body")).getText()).includes("PermissionRequest"),
    );
  });
  after(() => {
    const pending = visuals.filter((v) => v.status === "pending-review");
    writeFileSync(
      join(runDir, "acceptance.json"),
      JSON.stringify(
        {
          synthetic: true,
          visual: visuals.some((v) => v.status === "failed")
            ? "failed"
            : pending.length
              ? "pending-human-review"
              : "passed",
          pendingBaselines: pending.map((p) => p.name),
          manual: ["tray", "native-file-picker", "focus", "click-through"],
          realAgents: "not-exercised-by-this-suite",
        },
        null,
        2,
      ),
    );
    assert.equal(
      pending.length,
      0,
      `${pending.length} screenshot baselines need human review; candidates: ${runDir}`,
    );
  });
});
