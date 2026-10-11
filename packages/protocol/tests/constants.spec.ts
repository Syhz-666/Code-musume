import { describe, expect, it } from "vitest";
import {
  DEDUPE_WINDOW_MS,
  DONE_TTL_MS,
  ERROR_TTL_MS,
  INGEST_COALESCE_MS,
  LEASE_EXEMPT_STATES,
  LEASE_TTL_MS,
  MAX_SESSIONS,
  MIN_DWELL_MS,
  MIN_FORM_DWELL_MS,
  MUSUME_STATES,
  NOTIFICATION_TTL_MS,
  PRIORITY,
  SLEEPING_AFTER_IDLE_MS,
  STALE_AFTER_MS,
  TRANSIENT_TTL_MS,
} from "../src/index";

describe("constants（状态机规范 §5；数值漂移时此处先炸）", () => {
  it("11 个数值一次性快照（对齐规范 §5）", () => {
    expect({
      LEASE_TTL_MS,
      DONE_TTL_MS,
      NOTIFICATION_TTL_MS,
      ERROR_TTL_MS,
      MIN_DWELL_MS,
      DEDUPE_WINDOW_MS,
      SLEEPING_AFTER_IDLE_MS,
      STALE_AFTER_MS,
      INGEST_COALESCE_MS,
      MAX_SESSIONS,
      MIN_FORM_DWELL_MS,
    }).toEqual({
      LEASE_TTL_MS: 300_000,
      DONE_TTL_MS: 8_000,
      NOTIFICATION_TTL_MS: 20_000,
      ERROR_TTL_MS: 60_000,
      MIN_DWELL_MS: 4_000,
      DEDUPE_WINDOW_MS: 3_000,
      SLEEPING_AFTER_IDLE_MS: 600_000,
      STALE_AFTER_MS: 600_000,
      INGEST_COALESCE_MS: 200,
      MAX_SESSIONS: 8,
      MIN_FORM_DWELL_MS: 5_000,
    });
  });

  it("MUSUME_STATES：精确 8 态且有序（表序 = 优先级降序）", () => {
    expect(MUSUME_STATES).toEqual([
      "waiting-permission",
      "error",
      "notification",
      "done",
      "working",
      "thinking",
      "idle",
      "sleeping",
    ]);
    expect(new Set(MUSUME_STATES).size).toBe(8);
  });

  it("PRIORITY：精确快照 + 键集与状态集双向一致", () => {
    expect(PRIORITY).toEqual({
      "waiting-permission": 100,
      error: 90,
      notification: 85,
      done: 80,
      working: 60,
      thinking: 50,
      idle: 30,
      sleeping: 10,
    });
    expect(Object.keys(PRIORITY).sort()).toEqual([...MUSUME_STATES].sort());
  });

  it("优先级全序 + 值互不相等（相等会让聚合平局规则形同虚设）", () => {
    const states = [...MUSUME_STATES];
    for (const [i, s] of states.entries()) {
      const next = states[i + 1];
      if (next) expect(PRIORITY[s]).toBeGreaterThan(PRIORITY[next]);
    }
    expect(new Set(Object.values(PRIORITY)).size).toBe(MUSUME_STATES.length);
  });

  it("LEASE_EXEMPT_STATES = { waiting-permission }（规范 §4 豁免表）", () => {
    expect(LEASE_EXEMPT_STATES.size).toBe(1);
    expect([...LEASE_EXEMPT_STATES]).toEqual(["waiting-permission"]);
    expect(LEASE_EXEMPT_STATES.has("waiting-permission")).toBe(true);
    expect(LEASE_EXEMPT_STATES.has("error")).toBe(false);
  });

  it("TRANSIENT_TTL_MS：键集 = {done, notification, error}，值与独立常量同引用", () => {
    expect(Object.keys(TRANSIENT_TTL_MS).sort()).toEqual(["done", "error", "notification"]);
    expect(TRANSIENT_TTL_MS.done).toBe(DONE_TTL_MS);
    expect(TRANSIENT_TTL_MS.notification).toBe(NOTIFICATION_TTL_MS);
    expect(TRANSIENT_TTL_MS.error).toBe(ERROR_TTL_MS);
  });

  it("瞬态集 ⊆ 状态集；瞬态 ∩ 租约豁免 = ∅（互斥机制）", () => {
    for (const k of Object.keys(TRANSIENT_TTL_MS)) {
      expect(MUSUME_STATES).toContain(k);
    }
    expect(TRANSIENT_TTL_MS["waiting-permission"]).toBeUndefined();
  });

  it("非瞬态状态不误入瞬态表", () => {
    for (const k of ["working", "thinking", "idle", "sleeping"] as const) {
      expect(TRANSIENT_TTL_MS[k]).toBeUndefined();
    }
  });
});
