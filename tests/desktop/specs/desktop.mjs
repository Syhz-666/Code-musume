import assert from "node:assert/strict";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { PNG } from "pngjs";
import { assertEntryTiming, observePreviewEntry } from "../animation-observation.mjs";
import { startCodexPeer } from "../codex-peer.mjs";
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

async function restoreBanterConfig(previousConfig, mockBaseUrl) {
  const { config } = await nativeInvoke("get_banter_status");
  if (config.baseUrl === mockBaseUrl) {
    await nativeInvoke("save_banter", { config, key: null, deleteKey: true });
  }
  const restored = await nativeInvoke("save_banter", {
    config: previousConfig,
    key: null,
    deleteKey: false,
  });
  assert.deepEqual(restored.config, previousConfig);
}

describe("Actual desktop event chain and settings", () => {
  let initialConfig;
  let coldStartReady = false;
  before(async () => {
    // 共享 CI 机器上的渲染尖峰会让 IPC 往返超过 2s；放宽到与 wdio 的
    // waitforTimeout 一致，真正的卡死仍由 mocha 的用例超时兜底。
    await browser.setTimeout({ script: 10000 });
    await selectWindow("pet");
    const environment = await browser.execute(() => ({
      systemReducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      dpr: devicePixelRatio,
      userAgent: navigator.userAgent,
    }));
    console.log(`[environment] ${JSON.stringify(environment)}`);
    assert.equal(
      environment.systemReducedMotion,
      false,
      "Animation suite requires an explicit no-preference test environment",
    );
    await pet({ state: "idle" });
    assert.equal((await browser.$$("#app button")).length, 0);
    assert.equal(await (await browser.$(".character")).getAttribute("title"), "右键打开菜单");
    // Native menus are exercised separately through Windows computer use.
    await nativeInvoke("show_settings");
    await selectWindow("settings");
    initialConfig = await nativeInvoke("get_config");
  });
  beforeEach(async () => {
    // Each case owns its state, including after a failed screenshot/assertion.
    await selectWindow("settings");
    await nativeInvoke("set_config", { config: initialConfig });
    await nativeInvoke("set_animation_preview_enabled", { enabled: false });
    await nativeInvoke("set_animation_preview_state", { state: null });
    await nativeInvoke("reset_sessions");
    await browser.refresh();
    await selectWindow("pet");
    await browser.refresh();
    await pet({ state: "idle", animationMode: "real", renderer: initialConfig.renderer });
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
  it("configures banter and displays progress and completion summaries in both renderers", async function () {
    this.timeout(150_000);
    const requests = [];
    let completionReply = "\n 「登录过啦，保存还待查。」\r\n";
    const mock = createServer(async (req, res) => {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      const request = JSON.parse(Buffer.concat(chunks).toString());
      requests.push(request);
      const final = JSON.parse(request.messages.at(-1).content).本轮最终回复;
      res.setHeader("content-type", "application/json");
      res.end(
        JSON.stringify({
          choices: [
            {
              finish_reason: "stop",
              message: { content: final ? completionReply : "登录测试过啦。" },
            },
          ],
          usage: {
            prompt_tokens: 64,
            completion_tokens: 7,
            prompt_tokens_details: { cached_tokens: 16 },
          },
        }),
      );
    });
    await new Promise((resolve) => mock.listen(0, "127.0.0.1", resolve));
    let peer;
    const waitForRenderer = async (renderer) => {
      // Switching renderers reloads the pet WebView. Send hooks only after
      // the target renderer has submitted the latest idle projection.
      const { revision } = await nativeInvoke("get_projection");
      await pet({ state: "idle", renderer, revision, visualReady: true }, { prepare: false });
    };
    try {
      peer = await startCodexPeer();
      await tab("使用偏好");
      await (await browser.$('[data-testid="open-banter"]')).click();
      const panel = await browser.$('[data-testid="banter-settings"]');
      await panel.waitForDisplayed();
      const input = await panel.$$("input");
      assert.equal(await input[0].isSelected(), false);
      await input[0].click();
      await input[1].setValue(`http://127.0.0.1:${mock.address().port}/v1`);
      await input[2].setValue("mock-model");
      await input[3].setValue("synthetic-key");
      await input[4].setValue("小蓝");
      await (await panel.$("button=保存")).click();
      await browser.waitUntil(async () => (await panel.getText()).includes("已保存"));
      // WebView startup and atlas loading use real time. Keep the model bubble's
      // lifetime under test control while those operations complete.
      await nativeInvoke("e2e_set_clock_paused", { paused: true });
      for (const renderer of ["pixi", "dom"]) {
        const session = `codex-banter-${renderer}`;
        await selectWindow("settings");
        await nativeInvoke("reset_sessions");
        await nativeInvoke("set_config", { config: { ...initialConfig, renderer } });
        await nativeInvoke("e2e_advance_clock", { ms: 30_001 });
        await waitForRenderer(renderer);
        await hook(cx, "UserPromptSubmit", {}, session, "id118");
        await pet({ state: "thinking", renderer });
        await browser.waitUntil(() => peer.following(session));
        peer.replay(session);
        const bubble = await browser.$('.bubble[data-kind="banter"]');
        await bubble.waitForDisplayed();
        assert.equal(await bubble.getText(), "登录测试过啦。");
        const before = await nativeInvoke("get_banter_projection");
        await browser.waitUntil(async () => {
          const { banterPipeline } = await nativeInvoke("get_pipeline_diagnostics");
          return banterPipeline.trace.entries.some(
            (entry) => entry.revision === before.revision && entry.reason === "frame-submitted",
          );
        });
        const { capturedAtMs: now } = await nativeInvoke("get_pipeline_diagnostics");
        assert.equal(
          before.expiresAtMs - now,
          8000,
          "Unexpected summary lifetime before recreation",
        );
        await selectWindow("settings");
        await nativeInvoke("e2e_recreate_pet");
        await pet({ state: "thinking", renderer }, { prepare: false });
        const restored = await nativeInvoke("get_banter_projection");
        const { capturedAtMs: after } = await nativeInvoke("get_pipeline_diagnostics");
        console.log(
          `[banter-recreate] ${JSON.stringify({ renderer, before: before.revision, after: restored.revision, remainingMs: (restored.expiresAtMs ?? after) - after })}`,
        );
        assert.equal(after, now, "The test clock advanced during window recreation");
        assert.deepEqual(restored, before, "Window recreation changed the active summary");
        const restoredBubble = await browser.$('.bubble[data-kind="banter"]');
        await restoredBubble.waitForDisplayed();
        assert.equal(await restoredBubble.getText(), "登录测试过啦。");
        if (renderer === "pixi") {
          peer.requestApproval(session);
          await pet({ state: "waiting-permission", pending: 1 });
          assert.equal(await (await browser.$(".bubble")).getAttribute("data-kind"), "status");
        } else {
          await nativeInvoke("e2e_advance_clock", { ms: 7999 });
          assert.equal((await nativeInvoke("get_banter_projection")).revision, restored.revision);
          await nativeInvoke("e2e_advance_clock", { ms: 1 });
          const expired = await nativeInvoke("get_banter_projection");
          assert.equal(expired.text, null, "Summary survived its expiry");
          await selectWindow("settings");
          await nativeInvoke("e2e_recreate_pet");
          await pet({ state: "thinking", renderer }, { prepare: false });
          assert.deepEqual(await nativeInvoke("get_banter_projection"), expired);
          assert.equal(await (await browser.$(".bubble")).getAttribute("data-kind"), "");
        }
      }
      assert.equal(requests.length, 2);
      assert(
        requests.every((request) =>
          request.messages.at(-1).content.includes("redacted progress text"),
        ),
      );
      const report = await nativeInvoke("get_pipeline_diagnostics");
      assert.equal(report.codexApprovalSync.metrics.lastReceivedVersion, 13);
      assert.equal(report.codexApprovalSync.metrics.visibleMessages, 2);
      assert.equal(report.codexApprovalSync.messageCapture.progressMessages, 2);
      assert(report.codexApprovalSync.metrics.lastVisibleAtUnixMs > 0);
      assert.equal(report.banterOutcome, "总结成功");
      const progressTrace = report.banterPipeline.trace.entries.filter(
        (entry) => entry.kind === "progress",
      );
      const lastBubble = progressTrace.findLast((entry) => entry.reason === "frame-submitted");
      assert(lastBubble);
      assert(
        progressTrace.some(
          (entry) => entry.step === "request" && entry.requestId === lastBubble.requestId,
        ),
      );
      assert(
        progressTrace.some(
          (entry) => entry.step === "result" && entry.requestId === lastBubble.requestId,
        ),
      );
      assert(!JSON.stringify(report).includes("redacted progress text"));
      assert(!JSON.stringify(report).includes("synthetic-key"));
      assert(!JSON.stringify(report).includes("登录测试过啦。"));
      for (const [index, renderer] of ["pixi", "dom"].entries()) {
        const session = `complete-${renderer}`;
        await selectWindow("settings");
        await nativeInvoke("reset_sessions");
        await nativeInvoke("set_config", { config: { ...initialConfig, renderer } });
        await waitForRenderer(renderer);
        if (renderer === "dom") {
          await nativeInvoke("e2e_advance_clock", { ms: 30_001 });
          await hook(cc, "UserPromptSubmit", {}, "parallel-work", "parallel-turn");
          await pet({ state: "thinking", agent: cc, renderer });
          await nativeInvoke("e2e_banter_fixture", {
            progress: {
              agent: cc,
              session: "parallel-work",
              turn: "parallel-turn",
              id: "parallel-progress",
              text: "正在核对另一轮的测试。",
              kind: "progress",
            },
          });
          const progressBubble = await browser.$('.bubble[data-kind="banter"]');
          await progressBubble.waitForDisplayed();
          assert.equal(await progressBubble.getText(), "登录测试过啦。");
        }
        await hook(cx, "UserPromptSubmit", {}, session, "final-turn");
        await pet({ state: "thinking", renderer });
        await browser.waitUntil(() => peer.following(session));
        peer.beginFinalOnly(session);
        await browser.waitUntil(async () =>
          Object.values(
            (await nativeInvoke("get_pipeline_diagnostics")).codexApprovalSync.sessions,
          ).some((session) => session.status === "observed"),
        );
        if (renderer === "pixi") {
          peer.finishFinalOnly(session);
          await browser.waitUntil(
            async () =>
              (await nativeInvoke("get_pipeline_diagnostics")).codexApprovalSync.metrics
                .visibleMessages === 3,
          );
        }
        await hook(cx, "Stop", {}, session, "final-turn");
        await pet({ state: "done", renderer });
        assert.equal(await (await browser.$('.bubble[data-kind="status"]')).getText(), "这轮好了");
        if (renderer === "dom") peer.finishFinalOnly(session);
        await browser.waitUntil(
          async () =>
            (await nativeInvoke("get_pipeline_diagnostics")).banterCompletion.phase === "ready",
        );
        const previousBubble = await nativeInvoke("get_banter_projection");
        assert.equal(previousBubble.kind, renderer === "dom" ? "progress" : null);
        await nativeInvoke("e2e_advance_clock", { ms: 3000 });
        if (renderer === "dom") {
          await pet({ state: "thinking", agent: cc, renderer });
        } else {
          await pet({ state: "idle", renderer });
        }
        const complete = await browser.$('.bubble[data-kind="banter"]');
        await complete.waitForDisplayed();
        await browser.waitUntil(
          async () => (await complete.getText()) === "登录过啦，保存还待查。",
        );
        const completionBubble = await nativeInvoke("get_banter_projection");
        assert.equal(completionBubble.kind, "completion");
        if (renderer === "dom") {
          const { capturedAtMs } = await nativeInvoke("get_pipeline_diagnostics");
          assert(capturedAtMs < previousBubble.expiresAtMs);
        }
        await browser.waitUntil(async () => {
          const { banterPipeline } = await nativeInvoke("get_pipeline_diagnostics");
          return banterPipeline.trace.entries.some(
            (entry) =>
              entry.revision === completionBubble.revision && entry.reason === "frame-submitted",
          );
        });
        assert.equal(requests.length, 3 + 2 * index);
        assert.deepEqual(JSON.parse(requests.at(-1).messages.at(-1).content), {
          本轮最终回复: "登录测试通过，保存设置还没验证。",
        });
        if (renderer === "pixi") {
          await selectWindow("settings");
          await nativeInvoke("e2e_recreate_pet");
          await pet({ state: "idle", renderer }, { prepare: false });
          assert.equal(
            await (await browser.$('.bubble[data-kind="banter"]')).getText(),
            "登录过啦，保存还待查。",
          );
          await nativeInvoke("e2e_advance_clock", { ms: 8000 });
          await browser.waitUntil(
            async () => !(await browser.$('.bubble[data-kind="banter"]').isExisting()),
          );
        } else {
          await hook(cc, "UserPromptSubmit", {}, "next-work", "next-turn");
          await pet({ state: "thinking", agent: cc });
          await browser.waitUntil(
            async () => (await nativeInvoke("get_banter_projection")).kind === null,
          );
        }
      }
      await nativeInvoke("reset_sessions");
      await hook(cx, "UserPromptSubmit", {}, "missing-final", "no-text");
      await pet({ state: "thinking" });
      await hook(cx, "Stop", {}, "missing-final", "no-text");
      await pet({ state: "done" });
      await nativeInvoke("e2e_advance_clock", { ms: 2_001 });
      await browser.waitUntil(async () => {
        const { banterPipeline } = await nativeInvoke("get_pipeline_diagnostics");
        return banterPipeline.trace.entries.some(
          (entry) =>
            entry.kind === "completion" && entry.step === "turn" && entry.reason === "no-final",
        );
      });
      assert.equal(requests.length, 5);

      completionReply = "字".repeat(21);
      await nativeInvoke("reset_sessions");
      await hook(cx, "UserPromptSubmit", {}, "long-summary", "final-turn");
      await pet({ state: "thinking" });
      await browser.waitUntil(() => peer.following("long-summary"));
      peer.beginFinalOnly("long-summary");
      const previousFinals = (await nativeInvoke("get_pipeline_diagnostics")).codexApprovalSync
        .messageCapture.finalMessages;
      peer.finishFinalOnly("long-summary");
      await browser.waitUntil(async () => {
        const { codexApprovalSync } = await nativeInvoke("get_pipeline_diagnostics");
        return codexApprovalSync.messageCapture.finalMessages > previousFinals;
      });
      await hook(cx, "Stop", {}, "long-summary", "final-turn");
      await browser.waitUntil(async () => {
        const { banterPipeline } = await nativeInvoke("get_pipeline_diagnostics");
        return banterPipeline.trace.entries.some(
          (entry) => entry.kind === "completion" && entry.outputRejection === "too-long",
        );
      });
      await nativeInvoke("e2e_advance_clock", { ms: 3000 });
      await pet({ state: "idle" });
      assert.equal((await nativeInvoke("get_banter_projection")).text, null);
      assert.equal(requests.length, 6);
    } finally {
      await peer?.close();
      await selectWindow("settings");
      await nativeInvoke("e2e_set_clock_paused", { paused: false });
      const { config } = await nativeInvoke("get_banter_status");
      await nativeInvoke("save_banter", { config, key: null, deleteKey: true });
      mock.closeAllConnections();
      await new Promise((resolve) => mock.close(resolve));
    }
  });
  it("follows a live Codex transcript and prefers it over the legacy final message", async () => {
    const { config: previousConfig } = await nativeInvoke("get_banter_status");
    const fixture = readFileSync(
      new URL("../../../fixtures/codex-sync/rollout-0.171.jsonl", import.meta.url),
      "utf8",
    )
      .trim()
      .split(/\r?\n/);
    const session = JSON.parse(fixture[0]).payload.id;
    const turn = JSON.parse(fixture[1]).payload.turn_id;
    const day = join(runDir, "codex-home", "sessions", "2026", "11", "25");
    mkdirSync(day, { recursive: true });
    const transcriptPath = join(day, `rollout-2026-11-25T14-00-00-${session}.jsonl`);
    writeFileSync(transcriptPath, `${fixture.slice(0, 3).join("\n")}\n`);

    const requests = [];
    const mock = createServer(async (req, res) => {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      const { messages } = JSON.parse(Buffer.concat(chunks).toString());
      const input = JSON.parse(messages.at(-1).content);
      requests.push(input);
      const text = input.本轮最终回复 ? "登录过啦，保存还待查。" : "我在看登录测试呢。";
      res.setHeader("content-type", "application/json");
      res.end(JSON.stringify({ choices: [{ finish_reason: "stop", message: { content: text } }] }));
    });
    await new Promise((resolve) => mock.listen(0, "127.0.0.1", resolve));
    const baseUrl = `http://127.0.0.1:${mock.address().port}/v1`;
    let peer;
    const diagnostics = () => nativeInvoke("get_pipeline_diagnostics");
    const advance = (ms) => nativeInvoke("e2e_advance_clock", { ms });
    try {
      peer = await startCodexPeer();
      await nativeInvoke("e2e_set_clock_paused", { paused: true });
      await nativeInvoke("save_banter", {
        config: { enabled: true, baseUrl, model: "rollout-mock" },
        key: "synthetic-key",
        deleteKey: false,
      });
      await advance(30_001);
      const before = await diagnostics();
      await hook(cx, "UserPromptSubmit", { transcript_path: transcriptPath }, session, turn);
      await pet({ state: "thinking", agent: cx });
      await browser.waitUntil(
        async () =>
          (await diagnostics()).banterPipeline.codexTranscript.opened ===
          before.banterPipeline.codexTranscript.opened + 1,
      );
      await browser.waitUntil(() => peer.following(session));
      peer.beginFinalOnly(session, turn);
      peer.addProgress(session, "正在检查登录测试。");
      await browser.waitUntil(
        async () =>
          (await diagnostics()).codexApprovalSync.messageCapture.progressMessages >
          before.codexApprovalSync.messageCapture.progressMessages,
      );

      appendFileSync(transcriptPath, `${fixture[3]}\n${fixture.slice(3, 6).join("\n")}\n`);
      await advance(500);
      const bubble = await browser.$('.bubble[data-kind="banter"]');
      await bubble.waitForDisplayed();
      assert.equal(await bubble.getText(), "我在看登录测试呢。");
      assert.deepEqual(requests, [{ 可见进展: "正在检查登录测试。" }]);

      peer.finishFinalOnly(session, "旧同步返回的另一条最终回复。", 2);
      await browser.waitUntil(
        async () =>
          (await diagnostics()).codexApprovalSync.messageCapture.finalMessages >
          before.codexApprovalSync.messageCapture.finalMessages,
      );
      appendFileSync(transcriptPath, `${fixture.slice(6).join("\n")}\n`);
      await hook(cx, "Stop", { transcript_path: transcriptPath }, session, turn);
      await pet({ state: "done" });
      await advance(500);
      await browser.waitUntil(async () => (await diagnostics()).banterCompletion.phase === "ready");
      await advance(2500);
      await pet({ state: "idle" });
      await browser.waitUntil(async () => (await bubble.getText()) === "登录过啦，保存还待查。");
      assert.deepEqual(requests, [
        { 可见进展: "正在检查登录测试。" },
        { 本轮最终回复: "登录测试通过，保存设置还没验证。" },
      ]);
      const report = await diagnostics();
      const sources = report.banterPipeline.trace.entries;
      assert(sources.some((e) => e.source === "codex-transcript" && e.reason === "duplicate"));
      assert(sources.some((e) => e.source === "codex-sync-13" && e.reason === "duplicate"));
      assert(
        sources.some(
          (e) => e.source === "codex-sync-13" && e.reason === "preferred-final-already-captured",
        ),
      );
      assert(!JSON.stringify(report).includes("正在检查登录测试。"));
      assert(!JSON.stringify(report).includes(transcriptPath));
    } finally {
      try {
        await peer?.close();
        await selectWindow("settings");
        await restoreBanterConfig(previousConfig, baseUrl);
      } finally {
        mock.closeAllConnections();
        await new Promise((resolve) => mock.close(resolve));
        await nativeInvoke("e2e_set_clock_paused", { paused: false });
      }
    }
  });
  it("rolls for idle chatter, rests, wakes, and yields to work in both renderers", async function () {
    this.timeout(150_000);
    const status = () => nativeInvoke("get_banter_status");
    const { config: previousConfig } = await status();
    const requests = [];
    let releaseLate;
    const mock = createServer(async (req, res) => {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      const { messages } = JSON.parse(Buffer.concat(chunks).toString());
      const complete = (line) => {
        if (res.destroyed) return;
        res.setHeader("content-type", "application/json");
        res.end(
          JSON.stringify({ choices: [{ finish_reason: "stop", message: { content: line } }] }),
        );
      };
      requests.push(JSON.parse(messages.at(-1).content));
      if (requests.length % 2 === 0) releaseLate = () => complete("迟到的话。");
      else complete("想靠一会儿。");
    });
    await new Promise((resolve) => mock.listen(0, "127.0.0.1", resolve));
    const mockConfig = {
      baseUrl: `http://127.0.0.1:${mock.address().port}/v1`,
      model: "idle-mock",
    };
    const idle = async () => (await nativeInvoke("get_pipeline_diagnostics")).banterIdle;
    const advance = (ms) => nativeInvoke("e2e_advance_clock", { ms });
    try {
      await nativeInvoke("e2e_set_clock_paused", { paused: true });
      await nativeInvoke("save_banter", {
        config: mockConfig,
        key: "synthetic-key",
        deleteKey: false,
      });
      await tab("使用偏好");
      await (await browser.$('[data-testid="open-banter"]')).click();
      const panel = await browser.$('[data-testid="banter-settings"]');
      await panel.waitForDisplayed();
      const workToggle = await panel.$("label=开启工作碎碎念");
      const idleToggle = await panel.$("label=开启空闲碎碎念");
      assert.equal(await (await idleToggle.$("input")).isSelected(), false);
      await idleToggle.click();
      await (await panel.$("button=保存")).click();
      await browser.waitUntil(async () => (await status()).config.idleEnabled);
      assert.equal(await (await workToggle.$("input")).isSelected(), false);

      for (const [index, renderer] of ["pixi", "dom"].entries()) {
        await selectWindow("settings");
        const visible = { ...initialConfig, renderer, userHidden: false };
        await nativeInvoke("set_config", { config: visible });
        await nativeInvoke("reset_sessions");
        await nativeInvoke("e2e_idle_dice", { faces: [6, 1, 1, 1, 6, 6, 6] });
        await pet({ state: "idle", renderer, visualReady: true });
        await advance(30_000);
        const bubble = await browser.$('.bubble[data-kind="banter"]');
        await bubble.waitForDisplayed();
        assert.equal(await bubble.getText(), "想靠一会儿。");
        assert.deepEqual(requests[index * 2], { 场景: "空闲时自言自语", 最近说过: [] });

        await selectWindow("settings");
        await nativeInvoke("e2e_recreate_pet");
        await pet({ state: "idle", renderer, visualReady: true }, { prepare: false });
        await browser.waitUntil(async () =>
          (await browser.$(".bubble")).getText().then((text) => text === "想靠一会儿。"),
        );
        for (const rolls of [2, 3, 4]) {
          await advance(30_000);
          await browser.waitUntil(async () => (await idle()).rolls === rolls);
        }
        await pet({ state: "sleeping", renderer });
        await selectWindow("settings");
        await nativeInvoke("set_config", { config: { ...visible, userHidden: true } });
        await advance(60_000);
        assert.equal((await idle()).rolls, 4);
        await nativeInvoke("set_config", { config: visible });
        await pet({ state: "sleeping", renderer });
        await advance(15_000);
        await browser.waitUntil(async () => (await idle()).rolls === 5);
        assert.equal((await nativeInvoke("get_projection")).state, "sleeping");
        await advance(15_000);
        await pet({ state: "idle", renderer });
        assert.equal(requests.length, index * 2 + 1);
        await advance(30_000);
        await browser.waitUntil(() => requests.length === index * 2 + 2 && Boolean(releaseLate));
        assert.deepEqual(requests.at(-1).最近说过, ["想靠一会儿。"]);
        await hook(index === 0 ? cx : cc, "UserPromptSubmit");
        await pet({ state: "thinking", renderer });
        releaseLate();
        releaseLate = undefined;
        assert.equal((await nativeInvoke("get_banter_projection")).kind, null);
        assert.equal((await idle()).lastSkip, "interrupted");
      }
    } finally {
      try {
        await selectWindow("settings");
        await restoreBanterConfig(previousConfig, mockConfig.baseUrl);
      } finally {
        mock.closeAllConnections();
        await new Promise((resolve) => mock.close(resolve));
        await nativeInvoke("e2e_set_clock_paused", { paused: false });
      }
    }
  });
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
      assertEntryTiming(await observePreviewEntry("sleeping", "sleeping-enter"));
      await browser.waitUntil(async () => {
        const value = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return (
          value.segment === "sleeping" && value.frameFps === "24" && value.visualReady === "true"
        );
      });
      const sleepingFrames = await browser.execute(
        () =>
          new Promise((resolve) => {
            const frames = new Set();
            const onFrame = ({ detail }) => {
              if (detail.segment === "sleeping") frames.add(detail.frame);
            };
            window.addEventListener("code-musume:e2e-frame", onFrame);
            setTimeout(() => {
              window.removeEventListener("code-musume:e2e-frame", onFrame);
              resolve(frames.size);
            }, 650);
          }),
      );
      assert(sleepingFrames >= 8, "The sleeping loop must advance regularly, not render at 1 FPS");
      const sleepFrame = () =>
        browser.execute(() => Number(document.querySelector("#app").dataset.frame));
      await browser.waitUntil(async () => (await sleepFrame()) >= 78, { timeout: 5500 });
      await browser.waitUntil(async () => (await sleepFrame()) < 12, { timeout: 1500 });
      const wake = await observePreviewEntry("thinking", "thinking-enter", {
        exitSegment: "sleeping-exit",
      });
      // Deliberately read late: the recorded beginning must survive slow driver replies.
      await browser.pause(450);
      const lateView = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
      writeFileSync(
        join(runDir, `thinking-${renderer}-wake.json`),
        JSON.stringify({ renderer, ...wake, lateView }, null, 2),
      );
      console.log(
        `[animation-entry] ${JSON.stringify({ renderer, firstFrame: wake.samples[0]?.frame, firstReadyFrame: wake.firstReady?.frame, lateFrame: lateView.frame })}`,
      );
      assertEntryTiming(wake);
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
      await nativeInvoke("set_animation_preview_state", { state: "sleeping" });
      await manualPet("sleeping", "working");
      await browser.waitUntil(async () => {
        const value = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
        return value.segment === "sleeping" && value.frame === "0" && value.visualReady === "true";
      });
      const sleepPoster = Buffer.from(await browser.takeScreenshot(), "base64");
      assert.notDeepEqual(sleepPoster, frozen);
      await browser.pause(500);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), sleepPoster);
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
  it("loops waiting, plays brief gestures once and holds the error pose in either renderer", async () => {
    const view = () => browser.execute(() => ({ ...document.querySelector("#app").dataset }));
    for (const renderer of ["pixi", "dom"]) {
      await selectWindow("settings");
      const config = { ...initialConfig, renderer, reducedMotion: false };
      await nativeInvoke("set_config", { config });
      await nativeInvoke("set_animation_preview_enabled", { enabled: true });
      await nativeInvoke("set_animation_preview_state", { state: null });
      await pet({ renderer, state: "idle", animationMode: "real", visualReady: true });
      await browser.execute(() => {
        const host = document.querySelector("#app");
        delete host.dataset.waitingWrapped;
        let previous = -1;
        const onFrame = ({ detail }) => {
          if (detail.segment !== "waiting-permission") return;
          if (detail.frame < previous) {
            host.dataset.waitingWrapped = "true";
            window.removeEventListener("code-musume:e2e-frame", onFrame);
          }
          previous = detail.frame;
        };
        window.addEventListener("code-musume:e2e-frame", onFrame);
      });
      assertEntryTiming(
        await observePreviewEntry("waiting-permission", "waiting-permission-enter"),
      );
      await browser.waitUntil(
        async () => {
          const data = await view();
          return data.waitingWrapped === "true" && data.atlas === "attention.png";
        },
        { timeout: 6500 },
      );
      writeFileSync(
        join(runDir, `attention-${renderer}-waiting-loop.png`),
        Buffer.from(await browser.takeScreenshot(), "base64"),
      );

      const notification = await observePreviewEntry("notification", "notification-enter", {
        exitSegment: "waiting-permission-exit",
      });
      writeFileSync(
        join(runDir, `attention-${renderer}-notification-entry.json`),
        JSON.stringify({ renderer, ...notification }, null, 2),
      );
      assertEntryTiming(notification);
      await browser.waitUntil(async () => {
        const data = await view();
        return (
          data.segment === "notification" &&
          data.atlas === "attention.png" &&
          data.frame === "42" &&
          data.frameFps === "24" &&
          data.visualReady === "true"
        );
      });
      const rest = Buffer.from(await browser.takeScreenshot(), "base64");
      await browser.pause(500);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), rest);

      assertEntryTiming(await observePreviewEntry("done", "done-enter"));
      await browser.waitUntil(async () => {
        const data = await view();
        return (
          data.segment === "done" &&
          data.atlas === "thinking.png" &&
          data.frame === "35" &&
          data.frameFps === "24" &&
          data.visualReady === "true"
        );
      });
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), rest);
      await browser.pause(350);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), rest);

      assertEntryTiming(await observePreviewEntry("error", "error-enter"));
      await browser.waitUntil(async () => {
        const data = await view();
        return (
          data.segment === "error" &&
          data.atlas === "error.png" &&
          data.frame === "0" &&
          data.visualReady === "true"
        );
      });
      const concerned = Buffer.from(await browser.takeScreenshot(), "base64");
      assert.notDeepEqual(concerned, rest);
      await browser.pause(400);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), concerned);
      assertEntryTiming(
        await observePreviewEntry("notification", "notification-enter", {
          exitSegment: "error-exit",
        }),
      );

      await selectWindow("settings");
      await nativeInvoke("set_config", {
        config: { ...config, reducedMotion: true, manualAnimationEnabled: true },
      });
      await nativeInvoke("set_animation_preview_state", { state: "waiting-permission" });
      await selectWindow("pet");
      await browser.waitUntil(async () => {
        const data = await view();
        return (
          data.segment === "waiting-permission" &&
          data.atlas === "attention.png" &&
          data.frame === "0" &&
          data.animationRunning === "false" &&
          data.visualReady === "true"
        );
      });
      const raised = Buffer.from(await browser.takeScreenshot(), "base64");
      assert.notDeepEqual(raised, rest, "Reduced motion should show the waiting pose");
      writeFileSync(join(runDir, `attention-${renderer}-waiting-poster.png`), raised);

      await nativeInvoke("set_animation_preview_state", { state: "notification" });
      await browser.waitUntil(async () => {
        const data = await view();
        return (
          data.segment === "notification" &&
          data.frame === "0" &&
          data.animationRunning === "false" &&
          data.visualReady === "true"
        );
      });
      const poster = Buffer.from(await browser.takeScreenshot(), "base64");
      assert.notDeepEqual(poster, rest, "Reduced motion should show the raised reminder pose");
      await browser.pause(300);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), poster);

      await nativeInvoke("set_animation_preview_state", { state: "done" });
      await browser.waitUntil(async () => {
        const data = await view();
        return (
          data.segment === "done" &&
          data.frame === "0" &&
          data.animationRunning === "false" &&
          data.visualReady === "true"
        );
      });
      const nod = Buffer.from(await browser.takeScreenshot(), "base64");
      assert.notDeepEqual(nod, rest, "Reduced motion should show the nod pose");
      await browser.pause(300);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), nod);

      await nativeInvoke("set_animation_preview_state", { state: "error" });
      await browser.waitUntil(async () => {
        const data = await view();
        return (
          data.segment === "error" &&
          data.frame === "0" &&
          data.animationRunning === "false" &&
          data.visualReady === "true"
        );
      });
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), concerned);
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
    const config = await nativeInvoke("get_config");
    await nativeInvoke("set_animation_preview_enabled", { enabled: false });
    await nativeInvoke("set_config", { config: { ...config, reducedMotion: true } });
    await selectWindow("pet");
    await browser.refresh();
    await pet({ state: "working", agent: cx, form: "codex", animationMode: "real" });
    await browser.waitUntil(async () =>
      browser.execute(() => {
        const app = document.querySelector("#app");
        return (
          app.dataset.segment === "working" &&
          app.dataset.frame === "0" &&
          app.dataset.animationRunning === "false" &&
          app.dataset.visualReady === "true"
        );
      }),
    );
    await screenshot("both-restore-codex");
  });
  it("saves form settings through controls and preserves them across page reload", async () => {
    await hook(cx, "PreToolUse", { tool_use_id: "form-settings" });
    await pet({ state: "working", agent: cx, form: "codex" });
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
  it("plays both Codex costume changes and renders the eight Codex states", async () => {
    const view = () => browser.execute(() => ({ ...document.querySelector("#app").dataset }));
    const waitForChange = async (segment) => {
      await browser.waitUntil(
        async () => {
          const actual = await view();
          return (
            actual.segment === segment &&
            actual.atlas === "costume-change.webp" &&
            Number(actual.frame) > 0 &&
            actual.visualReady === "false"
          );
        },
        { timeoutMsg: `Did not play ${segment}` },
      );
    };
    for (const renderer of ["pixi", "dom"]) {
      await selectWindow("settings");
      await nativeInvoke("set_config", {
        config: { ...initialConfig, renderer, formLock: "default", reducedMotion: false },
      });
      await pet({ renderer, form: "default", atlas: "spritesheet.png" });
      const config = await nativeInvoke("get_config");
      await browser.execute(() => {
        window.costumeInterruption = [];
        const samples = window.costumeInterruption;
        window.addEventListener("code-musume:e2e-frame", (e) => {
          if (e.detail.phase === "costume" && samples.length < 300) {
            samples.push(e.detail);
          }
        });
      });
      await nativeInvoke("set_config", { config: { ...config, formLock: "codex" } });
      await waitForChange("form-to-codex");
      await nativeInvoke("set_config", { config });
      await pet({ renderer, form: "default", atlas: "spritesheet.png" });
      const interrupted = await browser.execute(() => window.costumeInterruption);
      assert(interrupted.every((frame) => frame.segment === "form-to-codex"));
      assert(
        interrupted.some((frame, i) => i > 0 && frame.frame < interrupted[i - 1].frame),
        `${renderer}: cancelled costume change should reverse from the painted frame`,
      );
      assert.equal(interrupted.at(-1)?.frame, 0);
      await nativeInvoke("set_config", { config: { ...config, formLock: "codex" } });
      await waitForChange("form-to-codex");
      await pet({ renderer, form: "codex", atlas: "codex-idle.png" });
      await browser.saveScreenshot(join(runDir, `codex-idle-${renderer}.png`));
      await nativeInvoke("set_animation_preview_enabled", { enabled: true });
      for (const [state, atlas] of Object.entries({
        idle: "codex-idle.png",
        thinking: "codex-thinking.webp",
        working: "codex-work.webp",
        "waiting-permission": "codex-work.webp",
        notification: "codex-reactions.webp",
        done: "codex-reactions.webp",
        error: "codex-rest.webp",
        sleeping: "codex-rest.webp",
      })) {
        await nativeInvoke("set_animation_preview_state", { state });
        await browser.waitUntil(
          async () => {
            const actual = await view();
            return (
              actual.displayState === state &&
              actual.visualState === state &&
              actual.atlas === atlas
            );
          },
          { timeoutMsg: `Wrong Codex artwork for ${state}` },
        );
      }
      await nativeInvoke("set_animation_preview_enabled", { enabled: false });
      await pet({ renderer, form: "codex", atlas: "codex-idle.png" });
      await nativeInvoke("set_config", { config: { ...config, formLock: "default" } });
      await waitForChange("form-to-default");
      await pet({ renderer, form: "default", atlas: "spritesheet.png" });
      await nativeInvoke("set_config", {
        config: { ...config, formLock: "codex", reducedMotion: true },
      });
      await pet({ renderer, form: "codex", atlas: "codex-idle.png", animationRunning: false });
    }
  });
  it("keeps Codex gestures held and sequences interrupted state changes", async () => {
    const clearFrames = () =>
      browser.execute(() => {
        window.codexPlaybackFrames = [];
      });
    const frames = () => browser.execute(() => window.codexPlaybackFrames ?? []);
    const waitFor = (check, message, timeout = 10000) =>
      browser.waitUntil(async () => check(await frames()), { timeout, timeoutMsg: message });
    const requestState = (state) => nativeInvoke("set_animation_preview_state", { state });
    for (const renderer of ["pixi", "dom"]) {
      await selectWindow("settings");
      const config = { ...initialConfig, renderer, formLock: "codex", reducedMotion: false };
      await nativeInvoke("set_config", { config });
      await pet({ renderer, form: "codex", atlas: "codex-idle.png" });
      await browser.execute(() => {
        window.codexPlaybackFrames = [];
        window.addEventListener("code-musume:e2e-frame", (e) =>
          window.codexPlaybackFrames.push(e.detail),
        );
      });
      await nativeInvoke("set_animation_preview_enabled", { enabled: true });
      for (const [state, frameCount] of [
        ["thinking", 90],
        ["waiting-permission", 70],
      ]) {
        await clearFrames();
        await requestState(state);
        await waitFor(
          (trace) => {
            const held = trace.filter((frame) => frame.segment === state && frame.phase === "loop");
            return held.filter((frame, i) => i && frame.frame < held[i - 1].frame).length >= 3;
          },
          `${renderer}: ${state} did not loop three times`,
          17000,
        );
        const held = (await frames()).filter((frame) => frame.segment === state);
        assert(held.every((frame) => frame.frames === frameCount && frame.phase === "loop"));
      }
      await clearFrames();
      await requestState("working");
      await waitFor(
        (trace) => trace.some((frame) => frame.segment === "working-enter"),
        "Working did not start",
      );
      const waitingExit = (await frames()).filter(
        (frame) => frame.segment === "waiting-permission-exit",
      );
      assert(waitingExit.length > 0 && waitingExit.at(-1).frame === waitingExit.at(-1).frames - 1);

      await requestState("idle");
      await waitFor((trace) => trace.at(-1)?.segment === "idle", "Did not return to idle");
      await clearFrames();
      await requestState("thinking");
      await waitFor((trace) => {
        const last = trace.at(-1);
        return last?.segment === "thinking-enter" && last.frame >= 8;
      }, "Thinking did not begin to raise her hand");
      await requestState("working");
      await waitFor(
        (trace) => trace.some((frame) => frame.segment === "working-enter"),
        "Interrupted thinking did not hand off",
      );
      const interrupted = await frames();
      const firstExit = interrupted.findIndex((frame) => frame.phase === "exit");
      assert(firstExit > 0);
      assert.equal(interrupted[firstExit].segment, "thinking-enter");
      assert.equal(interrupted[firstExit].frame, interrupted[firstExit - 1].frame);

      await clearFrames();
      await requestState("notification");
      await waitFor(
        (trace) => trace.at(-1)?.segment === "notification" && trace.at(-1).frame >= 8,
        "Wave did not start",
      );
      await requestState("thinking");
      await requestState("sleeping");
      await waitFor(
        (trace) => trace.some((frame) => frame.segment === "sleeping-enter"),
        "Latest state was not used after the wave",
      );
      const wave = await frames();
      assert(!wave.some((frame) => frame.segment === "thinking-enter"));
      const lastWave = wave.findLast((frame) => frame.segment === "notification");
      assert.equal(lastWave?.frame, 95);

      await clearFrames();
      await requestState("done");
      await waitFor(
        (trace) => trace.at(-1)?.segment === "done" && trace.at(-1).frame >= 15,
        "Completion did not reach the smile",
      );
      const previewConfig = await nativeInvoke("get_config");
      await nativeInvoke("set_config", { config: { ...previewConfig, formLock: "default" } });
      await waitFor(
        (trace) => trace.some((frame) => frame.segment === "form-to-default"),
        "Costume change did not start",
      );
      assert.equal((await frames()).findLast((frame) => frame.segment === "done")?.frame, 90);
      await requestState("working");
      await waitFor(
        (trace) => trace.some((frame) => frame.segment === "working-enter"),
        "Costume did not hand off to the latest state",
      );
      assert.equal(
        (await frames()).findLast((frame) => frame.segment === "form-to-default")?.frame,
        75,
      );
      await nativeInvoke("set_animation_preview_enabled", { enabled: false });
      await pet({ renderer, form: "default", state: "idle", atlas: "spritesheet.png" });
    }
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
    // Leave an explicitly saved DOM preference for the separate restart check.
    await choose('[data-testid="renderer-select"]', "dom");
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

    // Simulate Tauri resending a subscribe command after its first IPC response is lost.
    const initialRevision = await browser.execute(async () => {
      const { Channel, invoke } = window.__TAURI__.core;
      const args = {
        subscriptionId: crypto.randomUUID(),
        onUpdate: new Channel((projection) => {
          window.projectionReplayProbe.latest = projection;
        }),
      };
      window.projectionReplayProbe = { args, latest: null };
      return (await invoke("subscribe_projection", args)).revision;
    });
    await hook(cx, "Stop");
    await browser.waitUntil(async () =>
      browser.execute((revision) => {
        const latest = window.projectionReplayProbe.latest;
        return latest?.state === "done" && latest.revision > revision;
      }, initialRevision),
    );
    const replayRevision = await browser.execute(async () => {
      const { args } = window.projectionReplayProbe;
      return (await window.__TAURI__.core.invoke("subscribe_projection", args)).revision;
    });
    await nativeInvoke("reset_sessions");
    await browser.waitUntil(
      async () =>
        browser.execute((revision) => {
          const latest = window.projectionReplayProbe.latest;
          return latest?.state === "idle" && latest.revision > revision;
        }, replayRevision),
      { timeoutMsg: "Projection channel stopped delivering after subscription replay" },
    );
    await browser.execute(async () => {
      const { subscriptionId } = window.projectionReplayProbe.args;
      await window.__TAURI__.core.invoke("unsubscribe_projection", { subscriptionId });
      delete window.projectionReplayProbe;
    });
    coldStartReady = true;
  });
  after(() => {
    const pending = visuals.filter((v) => v.status === "pending-review");
    const failed = visuals.filter((v) => v.status === "failed");
    writeFileSync(
      join(runDir, "acceptance.json"),
      JSON.stringify(
        {
          synthetic: true,
          coldStartReady,
          visual: !visuals.length
            ? "not-run"
            : visuals.some((v) => v.status === "failed")
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
    assert.equal(
      failed.length,
      0,
      `Screenshot differences: ${failed.map((v) => v.name).join(", ")}`,
    );
  });
});
