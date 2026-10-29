import type { Clip } from "@code-musume/protocol";
import { describe, expect, it } from "vitest";
import { SpritePlayback } from "./sprite-playback";

const still: Clip = { row: 0, column: 0, frames: 1, fps: 1, looping: true };
const clips: Record<string, Clip> = {
  idle: { ...still, frames: 243, fps: 24 },
  thinking: { ...still, frames: 120, fps: 24 },
  "thinking-enter": { ...still, frames: 60, fps: 48, looping: false },
  "thinking-exit": { ...still, frames: 30, fps: 60, looping: false },
  working: still,
  sleeping: still,
};

describe("sprite presentation sequence", () => {
  it("crosses the common neutral between working and thinking and uses the latest interrupted target", () => {
    const animated = {
      ...clips,
      working: { ...still, image: "working.png", frames: 88, fps: 24 },
      "working-enter": { ...still, image: "working.png", frames: 36, fps: 48, looping: false },
      "working-exit": { ...still, image: "working.png", frames: 30, fps: 60, looping: false },
    };
    const player = new SpritePlayback();
    expect(player.sample(animated, "working", 0, true)).toMatchObject({
      key: "working-enter",
      index: 0,
    });
    expect(player.sample(animated, "working", 750, true)).toMatchObject({
      key: "working",
      index: 0,
    });
    expect(player.sample(animated, "thinking", 1000, true)).toMatchObject({
      key: "working-exit",
      ready: false,
    });
    expect(player.sample(animated, "thinking", 1500, true)).toMatchObject({
      key: "thinking-enter",
      index: 0,
    });
    player.sample(animated, "working", 1700, true);
    player.sample(animated, "sleeping", 1750, true);
    expect(player.sample(animated, "sleeping", 1900, true)).toMatchObject({
      key: "sleeping",
      ready: true,
    });
    expect(player.sample(animated, "working", 2000, false)).toMatchObject({
      key: "working",
      index: 0,
      phase: "loop",
    });
  });
  it("enters once then loops only the held thinking segment without restarting on real updates", () => {
    const player = new SpritePlayback();
    expect(player.sample(clips, "thinking", 0, true)).toMatchObject({
      key: "thinking-enter",
      index: 0,
      phase: "enter",
    });
    expect(player.sample(clips, "thinking", 1250, true)).toMatchObject({
      key: "thinking",
      index: 0,
      phase: "loop",
    });
    const loop = player.sample(clips, "thinking", 2250, true);
    expect(loop).toMatchObject({ index: 24, ready: true });
    expect(player.sample(clips, "thinking", 6250, true)).toMatchObject({
      key: "thinking",
      index: 0,
      token: loop?.token,
    });
  });

  it("plays the lowering gesture before returning to the common idle pose", () => {
    const player = new SpritePlayback();
    player.sample(clips, "thinking", 0, true);
    player.sample(clips, "thinking", 2000, true);
    expect(player.sample(clips, "idle", 2000, true)).toMatchObject({
      key: "thinking-exit",
      phase: "exit",
      index: 0,
      ready: false,
    });
    expect(player.sample(clips, "idle", 2250, true)).toMatchObject({
      key: "thinking-exit",
      index: 15,
      ready: false,
    });
    expect(player.sample(clips, "idle", 2500, true)).toMatchObject({
      key: "idle",
      index: 0,
      ready: true,
    });
  });

  it("reverses a partially raised hand instead of jumping to the held pose on interruption", () => {
    const player = new SpritePlayback();
    player.sample(clips, "thinking", 0, true);
    expect(player.sample(clips, "working", 200, true)).toMatchObject({
      key: "thinking-enter",
      index: 9,
      phase: "exit",
      ready: false,
    });
    const middle = player.sample(clips, "working", 300, true);
    expect(middle?.index).toBeGreaterThan(0);
    expect(middle?.index).toBeLessThan(9);
    expect(player.sample(clips, "working", 400, true)).toMatchObject({
      key: "working",
      ready: true,
    });
  });

  it("uses the latest requested state during an exit and never displays stale queued targets", () => {
    const player = new SpritePlayback();
    player.sample(clips, "thinking", 0, true);
    player.sample(clips, "thinking", 2000, true);
    player.sample(clips, "working", 2000, true);
    player.sample(clips, "sleeping", 2100, true);
    player.sample(clips, "idle", 2200, true);
    expect(player.sample(clips, "idle", 2500, true)).toMatchObject({
      state: "idle",
      key: "idle",
      ready: true,
    });
  });

  it("reduced motion cancels any entry/exit and shows the state's representative still", () => {
    const player = new SpritePlayback();
    player.sample(clips, "thinking", 0, true);
    player.sample(clips, "idle", 200, true);
    expect(player.sample(clips, "thinking", 220, false)).toMatchObject({
      key: "thinking",
      index: 0,
      phase: "loop",
      ready: true,
    });
    expect(player.sample(clips, "thinking", 1220, false)).toMatchObject({
      key: "thinking",
      index: 0,
    });
  });

  it("retains legacy clips and idle fallback when a package has no transition artwork", () => {
    const player = new SpritePlayback();
    expect(player.sample({ idle: still }, "error", 0, true)).toMatchObject({
      key: "idle",
      state: "error",
      ready: true,
    });
    expect(player.sample({ idle: still, working: still }, "working", 100, true)).toMatchObject({
      key: "working",
      ready: true,
    });
  });
});
