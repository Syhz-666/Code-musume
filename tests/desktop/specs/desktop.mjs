import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { PNG } from "pngjs";
import {
  choose,
  health,
  hook,
  makeCharacter,
  manualPet,
  nativeInvoke,
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
    await browser.setTimeout({ script: 2000 });
    await selectWindow("pet");
    await pet({ state: "idle" });
    assert.equal((await browser.$$("#app button")).length, 0);
    assert.equal(await (await browser.$(".character")).getAttribute("title"), "右键打开菜单");
    // Native menus are exercised separately through Windows computer use.
    await nativeInvoke("show_settings");
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
      await nativeInvoke("e2e_advance_clock", { ms: 4001 });
      await hook(cc, "PostToolUseFailure", { tool_use_id: "failed" });
      await pet({ state: "error" }, { latency: true });
      await screenshot(`${renderer}-error`);
      await nativeInvoke("e2e_advance_clock", { ms: 30001 });
      await hook(cc, "Stop");
      await pet({ state: "done" }, { latency: true });
      await screenshot(`${renderer}-done`);
      await nativeInvoke("e2e_advance_clock", { ms: 600001 });
      await pet({ state: "sleeping" });
      await screenshot(`${renderer}-sleeping`);
      await nativeInvoke("reset_sessions");
      await pet({ state: "idle" });
    });
  }
  it("plays the approved idle atlas across rows and loops, including manual preview and reduced motion", async () => {
    for (const renderer of ["pixi", "dom"]) {
      await selectWindow("settings");
      await nativeInvoke("reset_sessions");
      const config = await nativeInvoke("get_config");
      await nativeInvoke("set_config", { config: { ...config, renderer, reducedMotion: false } });
      await pet({ renderer, state: "idle", frameCount: 243, animationRunning: true });
      const initial = Number(
        await browser.execute(() => document.querySelector("#app").dataset.frame),
      );
      await browser.waitUntil(async () => {
        const data = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return Number(data.frame) > initial + 16 && Number(data.spriteRow) >= 1;
      });
      const first = Buffer.from(await browser.takeScreenshot(), "base64");
      writeFileSync(join(runDir, `animated-${renderer}-first.png`), first);
      await browser.waitUntil(
        async () =>
          Number(await browser.execute(() => document.querySelector("#app").dataset.frame)) >= 230,
        { timeout: 15000 },
      );
      const end = Number(await browser.execute(() => document.querySelector("#app").dataset.frame));
      await browser.waitUntil(
        async () =>
          Number(await browser.execute(() => document.querySelector("#app").dataset.frame)) < end,
        { timeout: 10000 },
      );
      const second = Buffer.from(await browser.takeScreenshot(), "base64");
      writeFileSync(join(runDir, `animated-${renderer}-wrapped.png`), second);
      assert.notDeepEqual(first, second, "Native sprite pixels should change during playback");
      await selectWindow("settings");
      await nativeInvoke("set_animation_preview_enabled", { enabled: true });
      await nativeInvoke("set_animation_preview_state", { state: "idle" });
      await hook(cc, "PreToolUse", { tool_use_id: `animated-${renderer}` });
      await manualPet("idle", "working");
      assert.equal(
        await browser.execute(() => document.querySelector("#app").dataset.frameCount),
        "243",
      );
      await browser.waitUntil(
        async () =>
          Number(await browser.execute(() => document.querySelector("#app").dataset.frame)) >= 16,
      );
      await nativeInvoke("set_animation_preview_state", { state: null });
      await pet({ state: "working", frameCount: 88, animationRunning: true });
      await selectWindow("settings");
      await nativeInvoke("set_animation_preview_enabled", { enabled: false });
      await nativeInvoke("reset_sessions");
      await nativeInvoke("set_config", { config: { ...config, renderer, reducedMotion: true } });
      await pet({ state: "idle", frame: 0, animationRunning: false });
      const frozen = Buffer.from(await browser.takeScreenshot(), "base64");
      await browser.pause(500);
      assert.equal(await browser.execute(() => document.querySelector("#app").dataset.frame), "0");
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), frozen);
    }
  });
  it("plays thinking entry/loop/exit smoothly and interrupts it without acknowledging a hidden real state", async () => {
    for (const renderer of ["pixi", "dom"]) {
      await selectWindow("settings");
      await nativeInvoke("reset_sessions");
      const config = await nativeInvoke("get_config");
      await nativeInvoke("set_config", { config: { ...config, renderer, reducedMotion: false } });
      await pet({ state: "idle", renderer, visualReady: true });
      await hook(cc, "UserPromptSubmit");
      await pet({ state: "thinking", segment: "thinking-enter", visualReady: true });
      writeFileSync(
        join(runDir, `thinking-${renderer}-entry.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await pet({
        state: "thinking",
        segment: "thinking",
        frameCount: 120,
        atlas: "thinking.png",
        visualReady: true,
      });
      writeFileSync(
        join(runDir, `thinking-${renderer}-held.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await browser.waitUntil(
        async () =>
          Number(await browser.execute(() => document.querySelector("#app").dataset.frame)) >= 110,
        { timeout: 10000 },
      );
      await browser.waitUntil(
        async () =>
          Number(await browser.execute(() => document.querySelector("#app").dataset.frame)) < 15,
        { timeout: 3000, interval: 30 },
      );
      assert.equal(
        await browser.execute(() => document.querySelector("#app").dataset.segment),
        "thinking",
      );
      writeFileSync(
        join(runDir, `thinking-${renderer}-loop.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );

      await nativeInvoke("set_animation_preview_enabled", { enabled: true });
      await nativeInvoke("set_animation_preview_state", { state: "thinking" });
      await hook(cc, "PreToolUse", { tool_use_id: `thinking-${renderer}` });
      await manualPet("thinking", "working");
      assert.equal(
        await browser.execute(() => document.querySelector("#app").dataset.segment),
        "thinking",
      );
      const hidden = await health();
      assert.notEqual(hidden.renderReceipt?.revision, hidden.projection.revision);
      await nativeInvoke("set_animation_preview_state", { state: null });
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => document.querySelector("#app").dataset.animationPhase)) ===
          "exit",
        { timeout: 1500, interval: 20 },
      );
      const exit = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
      assert.equal(exit.visualState, "thinking");
      assert.equal(exit.displayState, "working");
      assert.notEqual(
        exit.renderedRevision,
        exit.revision,
        "Do not acknowledge working while the old lowering gesture is still on screen",
      );
      writeFileSync(
        join(runDir, `thinking-${renderer}-exit.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await pet({ state: "working", segment: "working", visualReady: true });

      await nativeInvoke("set_animation_preview_state", { state: "thinking" });
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => document.querySelector("#app").dataset.segment)) ===
          "thinking",
        { timeout: 5000 },
      );
      await nativeInvoke("set_animation_preview_state", { state: "idle" });
      await browser.waitUntil(async () => {
        const value = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return (
          value.displayState === "idle" && value.segment === "idle" && value.visualReady === "true"
        );
      });
      writeFileSync(
        join(runDir, `thinking-${renderer}-returned.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await nativeInvoke("set_animation_preview_state", { state: "thinking" });
      await browser.waitUntil(
        async () => {
          const value = await browser.execute(() => ({
            ...document.querySelector("#app").dataset,
          }));
          return value.segment === "thinking-enter" && Number(value.frame) >= 10;
        },
        { interval: 20 },
      );
      for (const state of ["working", "sleeping", "idle"])
        await nativeInvoke("set_animation_preview_state", { state });
      await browser.waitUntil(async () => {
        const value = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return (
          value.displayState === "idle" && value.segment === "idle" && value.visualReady === "true"
        );
      });
      await nativeInvoke("set_animation_preview_state", { state: "thinking" });
      await nativeInvoke("set_animation_preview_state", { state: "sleeping" });
      await browser.waitUntil(async () => {
        const value = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return value.segment === "sleeping" && value.visualReady === "true";
      });
      await browser.pause(650);
      await nativeInvoke("set_animation_preview_state", { state: "thinking" });
      await browser.waitUntil(
        async () => {
          const value = await browser.execute(() => ({
            ...document.querySelector("#app").dataset,
          }));
          return value.segment === "thinking-enter" && value.visualReady === "true";
        },
        { interval: 20 },
      );
      assert(
        Number(await browser.execute(() => document.querySelector("#app").dataset.frame)) < 20,
        "Waking from low FPS must start near the entry's beginning",
      );
      writeFileSync(
        join(runDir, `thinking-${renderer}-wake.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await nativeInvoke("set_config", {
        config: { ...config, renderer, reducedMotion: true, manualAnimationEnabled: true },
      });
      await manualPet("thinking", "working");
      await browser.waitUntil(async () => {
        const value = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return value.segment === "thinking" && value.frame === "0" && value.blending === "false";
      });
      const frozen = Buffer.from(await browser.takeScreenshot(), "base64");
      writeFileSync(join(runDir, `thinking-${renderer}-reduced.png`), frozen);
      const image = PNG.sync.read(frozen);
      let characterPixels = 0;
      for (let y = 70; y < 190; y++)
        for (let x = 40; x < 120; x++) {
          const offset = (y * image.width + x) * 4;
          const [r, g, b, a] = image.data.subarray(offset, offset + 4);
          if (a > 32 && Math.max(r, g, b) - Math.min(r, g, b) > 20) characterPixels++;
        }
      assert(
        characterPixels > 150,
        "Reduced motion must show the thinking pose, not a blank frozen frame",
      );
      await browser.pause(500);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), frozen);
      await nativeInvoke("set_animation_preview_enabled", { enabled: false });
      await nativeInvoke("reset_sessions");
      await nativeInvoke("set_config", { config: { ...config, renderer, reducedMotion: true } });
      await pet({ state: "idle", visualReady: true });
    }
  });
  it("loops working and crosses thinking in both directions with a reversible interrupted entry", async () => {
    for (const renderer of ["pixi", "dom"]) {
      await selectWindow("settings");
      await nativeInvoke("reset_sessions");
      const config = await nativeInvoke("get_config");
      await nativeInvoke("set_config", { config: { ...config, renderer, reducedMotion: false } });
      await nativeInvoke("set_animation_preview_enabled", { enabled: true });
      await hook(cc, "UserPromptSubmit");
      await pet({ state: "thinking", segment: "thinking", renderer });
      await nativeInvoke("set_animation_preview_state", { state: "working" });
      await browser.waitUntil(
        async () => {
          const d = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
          return d.segment === "working-enter" && d.visualReady === "true";
        },
        { interval: 20 },
      );
      writeFileSync(
        join(runDir, `working-${renderer}-entry.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await manualPet("working", "thinking");
      await browser.waitUntil(async () => {
        const d = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return (
          d.segment === "working" &&
          d.frameCount === "88" &&
          d.atlas === "working.png" &&
          d.blending === "false"
        );
      });
      writeFileSync(
        join(runDir, `working-${renderer}-held.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await browser.waitUntil(
        async () =>
          Number(await browser.execute(() => document.querySelector("#app").dataset.frame)) >= 80,
        { timeout: 6000 },
      );
      await browser.waitUntil(
        async () =>
          Number(await browser.execute(() => document.querySelector("#app").dataset.frame)) < 10,
        { timeout: 2000, interval: 20 },
      );
      assert.equal(
        await browser.execute(() => document.querySelector("#app").dataset.segment),
        "working",
      );
      writeFileSync(
        join(runDir, `working-${renderer}-loop.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await hook(cc, "PreToolUse", { tool_use_id: `working-preview-${renderer}` });
      await manualPet("working", "working");
      const hidden = await health();
      assert.notEqual(hidden.renderReceipt?.revision, hidden.projection.revision);
      await nativeInvoke("set_animation_preview_state", { state: "thinking" });
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => document.querySelector("#app").dataset.segment)) ===
          "working-exit",
        { interval: 20 },
      );
      writeFileSync(
        join(runDir, `working-${renderer}-exit.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => document.querySelector("#app").dataset.segment)) ===
          "thinking",
      );
      await nativeInvoke("set_animation_preview_state", { state: "working" });
      await browser.waitUntil(
        async () => {
          const d = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
          return d.segment === "working-enter" && Number(d.frame) >= 8;
        },
        { interval: 20 },
      );
      for (const state of ["sleeping", "thinking", "idle"])
        await nativeInvoke("set_animation_preview_state", { state });
      await browser.waitUntil(async () => {
        const d = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return d.segment === "idle" && d.visualReady === "true";
      });
      await nativeInvoke("set_config", {
        config: { ...config, renderer, reducedMotion: true, manualAnimationEnabled: true },
      });
      await nativeInvoke("set_animation_preview_state", { state: "working" });
      await browser.waitUntil(async () => {
        const d = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return d.segment === "working" && d.frame === "0" && d.blending === "false";
      });
      const frozen = Buffer.from(await browser.takeScreenshot(), "base64");
      writeFileSync(join(runDir, `working-${renderer}-reduced.png`), frozen);
      const image = PNG.sync.read(frozen);
      let pixels = 0;
      for (let i = 0; i < image.data.length; i += 4) {
        const [r, g, b, a] = image.data.subarray(i, i + 4);
        if (a > 32 && Math.max(r, g, b) - Math.min(r, g, b) > 20) pixels++;
      }
      assert(pixels > 150, "Working reduced-motion poster must be visible");
      await browser.pause(400);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), frozen);
      await nativeInvoke("set_animation_preview_enabled", { enabled: false });
      await nativeInvoke("reset_sessions");
      await nativeInvoke("set_config", { config: { ...config, renderer, reducedMotion: true } });
      await pet({ state: "idle", visualReady: true });
    }
  });
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
    await nativeInvoke("reset_sessions");
    await hook(cc, "PreToolUse", { tool_use_id: "a" });
    await hook(cx, "PreToolUse", { tool_use_id: "b" });
    await hook(cc, "Stop");
    await pet({ state: "done", agent: cc });
    await nativeInvoke("e2e_advance_clock", { ms: 3001 });
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
    assert.equal(await (await browser.$('[data-testid="form-select"]')).getValue(), "claude");
    assert.equal(
      JSON.parse(readFileSync(join(runDir, "config/config.json"), "utf8")).formLock,
      "claude",
    );
    await choose('[data-testid="form-select"]', "");
    await (await browser.$("button=保存角色选择")).click();
    await pet({ form: "codex" });
  });
  it("manually previews all eight states without overriding real work in either renderer", async () => {
    await selectWindow("settings");
    await assert.rejects(
      nativeInvoke("set_animation_preview_state", { state: "working" }),
      /请先开启/,
    );
    await assert.rejects(nativeInvoke("set_animation_preview_state", { state: "invalid-state" }));
    for (const renderer of ["pixi", "dom"]) {
      await selectWindow("settings");
      await nativeInvoke("reset_sessions");
      await tab("使用偏好");
      await choose('[data-testid="renderer-select"]', renderer);
      await (await browser.$("button=保存使用偏好")).click();
      await pet({ renderer, state: "idle", animationMode: "real" });
      await tab("角色与形态");
      const toggle = await browser.$('[data-testid="animation-preview-enabled"]');
      assert.equal(await toggle.isSelected(), false);
      await toggle.click();
      await (await browser.$('[data-testid="animation-preview-state"]')).waitForDisplayed();
      await pet({ state: "idle", displayState: "idle", animationMode: "real" });
      for (const state of [
        "idle",
        "thinking",
        "working",
        "waiting-permission",
        "notification",
        "done",
        "error",
        "sleeping",
      ]) {
        await tab("角色与形态");
        await choose('[data-testid="animation-preview-state"]', state);
        await manualPet(state, "idle");
        writeFileSync(
          join(runDir, `manual-${renderer}-${state}.png`),
          Buffer.from(await browser.takeScreenshot(), "base64"),
        );
      }
      await hook(cc, "UserPromptSubmit");
      await manualPet("sleeping", "thinking");
      await hook(cc, "PermissionRequest", { tool_use_id: "manual-wait" });
      await manualPet("sleeping", "waiting-permission");
      const waiting = await health();
      assert.equal(waiting.projection.pendingCount, 1);
      assert.notEqual(waiting.renderReceipt?.revision, waiting.projection.revision);
      await selectWindow("pet");
      const falseReceipt = await nativeInvoke("report_rendered", {
        instanceId: waiting.projection.instanceId,
        revision: waiting.projection.revision,
      });
      assert.equal(falseReceipt, false);
      await tab("角色与形态");
      assert.match(
        await (await browser.$('[data-testid="animation-preview-real-state"]')).getText(),
        /等你处理/,
      );
      await (await browser.$('[data-testid="animation-preview-panel"]')).scrollIntoView();
      writeFileSync(
        join(runDir, `settings-manual-${renderer}.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );
      await browser.refresh();
      await tab("角色与形态");
      assert.equal(
        await (await browser.$('[data-testid="animation-preview-state"]')).getValue(),
        "sleeping",
      );
      if (renderer === "dom") {
        await browser.closeWindow();
        await manualPet("sleeping", "waiting-permission");
        await nativeInvoke("show_settings");
        await tab("角色与形态");
        assert.equal(
          await (await browser.$('[data-testid="animation-preview-state"]')).getValue(),
          "sleeping",
        );
      }
      await selectWindow("pet");
      await browser.refresh();
      await manualPet("sleeping", "waiting-permission");
      await hook(cc, "Stop");
      await manualPet("sleeping", "done");
      await tab("角色与形态");
      await (await browser.$('[data-testid="animation-preview-return"]')).click();
      await pet({ state: "done", displayState: "done", animationMode: "real" });
      const returned = await health();
      assert.equal(returned.projection.pendingCount, 0);
      assert.equal(returned.renderReceipt?.revision, returned.projection.revision);
      await tab("角色与形态");
      assert.equal(
        await (await browser.$('[data-testid="animation-preview-enabled"]')).isSelected(),
        true,
      );
      await choose('[data-testid="animation-preview-state"]', "working");
      await manualPet("working", "done");
      await tab("角色与形态");
      await (await browser.$('[data-testid="animation-preview-enabled"]')).click();
      await pet({ state: "done", displayState: "done", animationMode: "real" });
      assert.equal(
        JSON.parse(readFileSync(join(runDir, "config/config.json"), "utf8")).manualAnimationEnabled,
        false,
      );
    }
  });
  it("keeps preview controls separate from unsaved settings and reports missing clips", async () => {
    await tab("角色与形态");
    await choose('[data-testid="form-select"]', "claude");
    await (await browser.$('[data-testid="animation-preview-enabled"]')).click();
    await (await browser.$('[data-testid="animation-preview-state"]')).waitForDisplayed();
    const saved = JSON.parse(readFileSync(join(runDir, "config/config.json"), "utf8"));
    assert.equal(saved.formLock, null);
    assert.equal(saved.manualAnimationEnabled, true);
    await choose('[data-testid="form-select"]', "");
    const directory = makeCharacter();
    const path = join(directory, "musume.json");
    const manifest = JSON.parse(readFileSync(path, "utf8"));
    for (const form of manifest.forms) form.clips = { idle: form.clips.idle };
    writeFileSync(path, JSON.stringify(manifest));
    const imported = await nativeInvoke("import_character", { source: directory });
    const config = await nativeInvoke("get_config");
    await nativeInvoke("set_config", { config: { ...config, characterId: imported.id } });
    await browser.refresh();
    await tab("角色与形态");
    await choose('[data-testid="animation-preview-state"]', "error");
    await (await browser.$('[data-testid="animation-preview-missing"]')).waitForDisplayed();
    assert.match(
      await (await browser.$('[data-testid="animation-preview-missing"]')).getText(),
      /显示待机/,
    );
    await (await browser.$('[data-testid="animation-preview-return"]')).click();
    await (await browser.$('[data-testid="animation-preview-enabled"]')).click();
    await choose('[data-testid="character-select"]', "builtin");
    await (await browser.$("button=保存角色选择")).click();
    await selectWindow("settings");
    await nativeInvoke("reset_sessions");
    await pet({ character: "builtin", state: "idle", animationMode: "real" });
  });
  it("imports declared atlas pages and rejects undeclared page requests", async () => {
    await tab("角色与形态");
    const directory = makeCharacter();
    const path = join(directory, "musume.json");
    const manifest = JSON.parse(readFileSync(path, "utf8"));
    writeFileSync(join(directory, "thinking.png"), readFileSync(join(directory, "pet.png")));
    for (const form of manifest.forms) form.clips.thinking.image = "thinking.png";
    writeFileSync(path, JSON.stringify(manifest));
    const imported = await nativeInvoke("import_character", { source: directory });
    assert(existsSync(join(runDir, "config/characters", imported.id, "thinking.png")));
    const page = await nativeInvoke("get_character_asset", {
      id: imported.id,
      form: "default",
      image: "thinking.png",
    });
    assert.equal(
      page,
      `data:image/png;base64,${readFileSync(join(directory, "thinking.png")).toString("base64")}`,
    );
    await assert.rejects(
      nativeInvoke("get_character_asset", {
        id: imported.id,
        form: "default",
        image: "../pet.png",
      }),
    );
    await assert.rejects(
      nativeInvoke("get_character_asset", {
        id: imported.id,
        form: "default",
        image: "missing.png",
      }),
    );
    const config = await nativeInvoke("get_config");
    await nativeInvoke("set_config", {
      config: { ...config, characterId: imported.id, reducedMotion: true },
    });
    await nativeInvoke("set_animation_preview_enabled", { enabled: true });
    await nativeInvoke("set_animation_preview_state", { state: "thinking" });
    await selectWindow("pet");
    await browser.waitUntil(async () => {
      const value = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
      return (
        value.character === imported.id &&
        value.atlas === "thinking.png" &&
        value.visualReady === "true"
      );
    });
    await nativeInvoke("set_animation_preview_enabled", { enabled: false });
    await nativeInvoke("set_config", {
      config: { ...config, characterId: "builtin", reducedMotion: true },
    });
    await nativeInvoke("reset_sessions");
    await pet({ character: "builtin", state: "idle" });
  });
  it("imports real image data and shows an actionable error for invalid input", async () => {
    await hook(cx, "PreToolUse", { tool_use_id: "import-working" });
    await pet({ state: "working", agent: cx, form: "codex" });
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
    await tab("角色与形态");
    await nativeInvoke("set_animation_preview_enabled", { enabled: true });
    await (await browser.$('[data-testid="animation-preview-state"]')).waitForDisplayed();
    await choose('[data-testid="animation-preview-state"]', "sleeping");
    await manualPet("sleeping", "waiting-permission");
    await selectWindow("settings");
    await nativeInvoke("e2e_recreate_pet");
    // WDIO 1.5.0 retains its async message handler by label across destruction.
    // The standard synchronous WebDriver channel reads the newly created WebView directly.
    await manualPet("sleeping", "waiting-permission", { prepare: false });
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
