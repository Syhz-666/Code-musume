import assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { hook, nativeInvoke, pet, runDir, selectWindow } from "../helpers.mjs";

describe("System reduced-motion preference", () => {
  it("keeps both renderers still when the application animation preference is off", async () => {
    await browser.setTimeout({ script: 2000 });
    await selectWindow("pet");
    assert.equal(
      await browser.execute(() => matchMedia("(prefers-reduced-motion: reduce)").matches),
      true,
      "This suite must actually emulate the system reduced-motion preference",
    );
    await nativeInvoke("show_settings");
    const results = [];
    for (const renderer of ["pixi", "dom"]) {
      await selectWindow("settings");
      const config = await nativeInvoke("get_config");
      await nativeInvoke("set_config", { config: { ...config, renderer, reducedMotion: false } });
      await nativeInvoke("reset_sessions");
      await pet({
        renderer,
        state: "idle",
        frame: 0,
        animationRunning: false,
        configuredReducedMotion: false,
        systemReducedMotion: true,
      });
      await hook("claude-code", "UserPromptSubmit");
      await pet(
        {
          state: "thinking",
          segment: "thinking",
          frame: 0,
          animationRunning: false,
          configuredReducedMotion: false,
          systemReducedMotion: true,
        },
        { latency: true },
      );
      const frozen = Buffer.from(await browser.takeScreenshot(), "base64");
      await browser.pause(400);
      assert.deepEqual(Buffer.from(await browser.takeScreenshot(), "base64"), frozen);
      const view = await browser.execute(() => ({ ...document.querySelector("#app").dataset }));
      results.push({ renderer, view, pixelsUnchanged: true });
      writeFileSync(join(runDir, `system-motion-${renderer}.png`), frozen);
    }
    writeFileSync(join(runDir, "system-motion-results.json"), JSON.stringify(results, null, 2));
  });
});
