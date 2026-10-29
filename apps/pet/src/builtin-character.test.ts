import { STATE_LABELS } from "@code-musume/protocol";
import { describe, expect, it } from "vitest";
import { builtinAssetUrl, builtinManifest, builtinSpriteUrl } from "./builtin-character";

describe("bundled homewear character", () => {
  it("keeps default and agent forms compatible with existing configurations", () => {
    expect(builtinManifest.id).toBe("builtin");
    expect(builtinManifest.defaultForm).toBe("default");
    expect(builtinManifest.agentForms).toEqual({ "claude-code": "claude", codex: "codex" });
    expect(builtinManifest.forms.map((form) => form.id)).toEqual(["default", "claude", "codex"]);
    expect(builtinSpriteUrl).toContain("spritesheet.png");
  });

  it("keeps resolution and all states with separate thinking and working atlases", () => {
    for (const form of builtinManifest.forms) {
      expect(Object.keys(form.clips).sort()).toEqual(
        [
          ...Object.keys(STATE_LABELS),
          "thinking-enter",
          "thinking-exit",
          "working-enter",
          "working-exit",
        ].sort(),
      );
      expect(form.cellWidth).toBe(192);
      expect(form.cellHeight).toBe(208);
      expect(form.columns).toBe(16);
      expect(form.clips.idle).toEqual({ row: 0, column: 0, frames: 243, fps: 24, looping: true });
      expect(form.clips.thinking).toMatchObject({
        image: "thinking.png",
        frames: 120,
        fps: 24,
        looping: true,
      });
      expect(form.clips["thinking-enter"]).toMatchObject({ frames: 60, fps: 48, looping: false });
      expect(form.clips["thinking-exit"]).toMatchObject({ frames: 30, fps: 60, looping: false });
      expect(builtinAssetUrl("thinking.png")).toContain("thinking.png");
      expect(form.clips.working).toMatchObject({
        image: "working.png",
        frames: 88,
        fps: 24,
        looping: true,
      });
      expect(form.clips["working-enter"]).toMatchObject({ frames: 36, fps: 48, looping: false });
      expect(form.clips["working-exit"]).toMatchObject({ frames: 30, fps: 60, looping: false });
      expect(builtinAssetUrl("working.png")).toContain("working.png");
      expect(() => builtinAssetUrl("missing.png")).toThrow();
      const occupied = new Set<string>();
      for (const [state, clip] of Object.entries(form.clips)) {
        if (!state.startsWith("thinking") && !state.startsWith("working") && state !== "idle")
          expect(clip.frames).toBe(1);
        for (let i = 0; i < clip.frames; i++) {
          const slot = `${clip.image ?? form.image}:${clip.row * form.columns + clip.column + i}`;
          expect(occupied.has(slot)).toBe(false);
          occupied.add(slot);
        }
      }
      expect(occupied.size).toBe(612);
    }
  });
});
