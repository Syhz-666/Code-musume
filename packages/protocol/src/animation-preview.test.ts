import { describe, expect, it } from "vitest";
import { AnimationPreviewStore } from "./view";

describe("animation preview ordering", () => {
  it("accepts initial revision zero and does not restore an already cancelled selection", () => {
    const store = new AnimationPreviewStore();
    expect(store.accept({ enabled: false, state: null, revision: 0 })).toBe(true);
    expect(store.accept({ enabled: true, state: "sleeping", revision: 2 })).toBe(true);
    expect(store.accept({ enabled: false, state: null, revision: 3 })).toBe(true);
    expect(store.accept({ enabled: true, state: "sleeping", revision: 2 })).toBe(false);
    expect(store.accept({ enabled: true, state: "working", revision: 3 })).toBe(false);
    expect(store.current?.enabled).toBe(false);
    expect(store.current?.state).toBe(null);
  });

  it("rejects invalid revisions and a selection when the feature is disabled", () => {
    const store = new AnimationPreviewStore();
    for (const revision of [-1, Number.NaN, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
      expect(store.accept({ enabled: true, state: null, revision })).toBe(false);
    }
    expect(store.accept({ enabled: false, state: "idle", revision: 1 })).toBe(false);
    expect(store.current).toBe(null);
  });
});
