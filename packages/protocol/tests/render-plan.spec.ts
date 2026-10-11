import { describe, expect, it } from "vitest";
import { ACTION_KEYS, BUBBLE_KEYS, DEFAULT_RENDER_PLAN } from "../src/index";

describe("render-plan（事件协议 §6）", () => {
  it("ACTION_KEYS：8 值有序、无重复", () => {
    expect(ACTION_KEYS).toEqual([
      "idle",
      "thinking",
      "working",
      "waiting_permission",
      "notification",
      "done",
      "error",
      "sleeping",
    ]);
    expect(new Set(ACTION_KEYS).size).toBe(8);
  });

  it("BUBBLE_KEYS：4 值且 ⊆ ACTION_KEYS", () => {
    expect(BUBBLE_KEYS).toEqual(["waiting_permission", "notification", "done", "error"]);
    for (const k of BUBBLE_KEYS) expect(ACTION_KEYS).toContain(k);
  });

  it("DEFAULT_RENDER_PLAN 精确快照（toEqual 顺带锁定不含 bubble/transition/sfx）", () => {
    expect(DEFAULT_RENDER_PLAN).toEqual({
      animation: "idle",
      form: "default",
      slots: [],
      intensity: 0.3,
    });
  });

  it("默认计划合法性：动作键在枚举内、form 非空、intensity ∈ [0,1]", () => {
    expect(ACTION_KEYS).toContain(DEFAULT_RENDER_PLAN.animation);
    expect(DEFAULT_RENDER_PLAN.form.length).toBeGreaterThan(0);
    expect(Array.isArray(DEFAULT_RENDER_PLAN.slots)).toBe(true);
    expect(DEFAULT_RENDER_PLAN.slots).toHaveLength(0);
    expect(DEFAULT_RENDER_PLAN.intensity).toBeGreaterThanOrEqual(0);
    expect(DEFAULT_RENDER_PLAN.intensity).toBeLessThanOrEqual(1);
  });
});
