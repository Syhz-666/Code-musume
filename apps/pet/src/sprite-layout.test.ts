import { describe, expect, it } from "vitest";
import { spriteFramePosition } from "./sprite-layout";

describe("sprite atlas clips", () => {
  it("continues the full idle clip through row boundaries without entering static states", () => {
    const idle = { row: 0, column: 0 };
    expect(spriteFramePosition(16, idle, 15)).toEqual({ row: 0, column: 15 });
    expect(spriteFramePosition(16, idle, 16)).toEqual({ row: 1, column: 0 });
    expect(spriteFramePosition(16, idle, 242)).toEqual({ row: 15, column: 2 });
    expect(spriteFramePosition(16, { row: 15, column: 3 }, 0)).toEqual({ row: 15, column: 3 });
  });

  it("keeps legacy single-row atlas coordinates", () => {
    expect(spriteFramePosition(8, { row: 7, column: 0 }, 5)).toEqual({ row: 7, column: 5 });
  });
});
