import { describe, expect, it } from "vitest";
import { CostumePlayback, WaitingPresentation } from "./waiting-presentation";

const waiting = { characterId: "builtin", formId: "codex", state: "waiting-permission" } as const;
const working = { ...waiting, formId: "claude", state: "working" } as const;

describe("approval animation", () => {
  it("keeps the waiting pose through a brief release and does not queue a costume change", () => {
    const visual = new WaitingPresentation();
    expect(visual.resolve(waiting, 0, false)).toBe(waiting);
    expect(visual.resolve(working, 10, false)).toBe(waiting);
    expect(visual.resolve(working, 220, false)).toBe(waiting);
    expect(visual.resolve(waiting, 230, false)).toBe(waiting);
    expect(visual.resolve(waiting, 1000, false)).toBe(waiting);
  });

  it("waits briefly for a cross-form approval, then commits a sustained change", () => {
    const visual = new WaitingPresentation();
    visual.resolve(working, 0, false);
    expect(visual.resolve(waiting, 10, false)).toBe(working);
    expect(visual.resolve(waiting, 249, false)).toBe(working);
    expect(visual.resolve(waiting, 250, false)).toBe(waiting);
    expect(visual.resolve(working, 500, false)).toBe(waiting);
    const thinking = { ...working, state: "thinking" } as const;
    expect(visual.resolve(thinking, 739, false)).toBe(waiting);
    expect(visual.resolve(thinking, 740, false)).toBe(thinking);
  });

  it("does not delay a same-form approval, an error, or a manual pose", () => {
    const visual = new WaitingPresentation();
    visual.resolve({ ...waiting, state: "working" }, 0, false);
    expect(visual.resolve(waiting, 1, false)).toBe(waiting);
    const error = { ...working, state: "error" } as const;
    expect(visual.resolve(error, 2, false)).toBe(error);
    expect(visual.resolve(waiting, 3, true)).toBe(waiting);
    expect(visual.resolve(working, 4, true)).toBe(working);
  });
});

describe("costume animation", () => {
  it("returns from the last painted frame without showing the cancelled destination", () => {
    const change = new CostumePlayback(0, 8, 25);
    expect(change.sample(160, false)).toEqual({ index: 4, done: false });
    change.markPresented(4);
    expect(change.sample(360, true)).toEqual({ index: 4, done: false });
    expect(change.sample(440, true)).toEqual({ index: 2, done: false });
    change.markPresented(2);
    expect(change.sample(560, true)).toEqual({ index: 0, done: false });
    change.markPresented(0);
    expect(change.sample(561, true)).toEqual({ index: 0, done: true });
  });

  it("continues forward from the last painted reverse frame when the destination wins again", () => {
    const change = new CostumePlayback(0, 8, 25);
    change.markPresented(change.sample(160, false).index);
    change.sample(180, true);
    change.markPresented(change.sample(260, true).index);
    expect(change.sample(300, false)).toEqual({ index: 2, done: false });
    expect(change.sample(600, false)).toEqual({ index: 7, done: false });
    change.markPresented(7);
    expect(change.sample(601, false)).toEqual({ index: 7, done: true });
  });
});
