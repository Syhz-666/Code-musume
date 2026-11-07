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

  it("keeps the shared resolution and non-overlapping clips for all states", () => {
    for (const form of builtinManifest.forms) {
      expect(Object.keys(form.clips).sort()).toEqual(
        [
          ...Object.keys(STATE_LABELS),
          "thinking-enter",
          "thinking-exit",
          "working-enter",
          "working-exit",
          "waiting-permission-enter",
          "waiting-permission-exit",
          "notification-enter",
          "done-enter",
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
      expect(form.clips["waiting-permission"]).toMatchObject({
        image: "attention.png",
        frames: 85,
        fps: 24,
        looping: true,
      });
      expect(form.clips["waiting-permission-enter"]).toMatchObject({ frames: 37, looping: false });
      expect(form.clips["waiting-permission-exit"]).toMatchObject({ frames: 24, looping: false });
      expect(form.clips["notification-enter"]).toEqual({
        image: "attention.png",
        row: 9,
        column: 2,
        frames: 23,
        fps: 24,
        looping: false,
      });
      expect(form.clips.notification).toEqual({
        image: "attention.png",
        row: 10,
        column: 9,
        frames: 43,
        fps: 24,
        looping: false,
      });
      expect(form.clips["done-enter"]).toEqual({
        row: 15,
        column: 5,
        frames: 25,
        fps: 24,
        looping: false,
      });
      expect(form.clips.done).toEqual({
        image: "thinking.png",
        row: 13,
        column: 2,
        frames: 36,
        fps: 24,
        looping: false,
      });
      expect(builtinAssetUrl("attention.png")).toContain("attention.png");
      expect(() => builtinAssetUrl("missing.png")).toThrow();
      const occupied = new Set<string>();
      for (const [state, clip] of Object.entries(form.clips)) {
        if (["error", "sleeping"].includes(state)) {
          expect(clip.frames).toBe(1);
        }
        for (let i = 0; i < clip.frames; i++) {
          const slot = `${clip.image ?? form.image}:${clip.row * form.columns + clip.column + i}`;
          expect(occupied.has(slot)).toBe(false);
          occupied.add(slot);
        }
      }
      expect(occupied.size).toBe(882);
    }
  });
});
