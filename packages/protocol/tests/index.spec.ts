import { describe, expect, it } from "vitest";
import * as protocol from "../src/index";

/** 运行时导出面（类型导出被擦除、不计入）。新增导出必须同步此清单——刻意的脆弱。 */
const EXPECTED_EXPORTS = [
  "ACTION_KEYS",
  "BUBBLE_KEYS",
  "CANON_KINDS",
  "DEFAULT_RENDER_PLAN",
  "DEDUPE_WINDOW_MS",
  "DONE_TTL_MS",
  "ENVELOPE_MAX_PAYLOAD_BYTES",
  "ERROR_TTL_MS",
  "INGEST_COALESCE_MS",
  "LEASE_EXEMPT_STATES",
  "LEASE_TTL_MS",
  "MAX_SESSIONS",
  "MIN_DWELL_MS",
  "MIN_FORM_DWELL_MS",
  "MUSUME_STATES",
  "NOTIFICATION_TTL_MS",
  "NOTIFICATION_TYPES",
  "PERMISSION_MODES",
  "PRIORITY",
  "PROTOCOL_VERSION",
  "SESSION_END_REASONS",
  "SESSION_SOURCES",
  "SLEEPING_AFTER_IDLE_MS",
  "STALE_AFTER_MS",
  "STOP_ERRORS",
  "TRANSIENT_TTL_MS",
  "asNotificationType",
  "asPermissionMode",
  "asSessionEndReason",
  "asSessionSource",
  "asStopError",
  "isFiniteNumber",
  "isNonEmptyString",
  "isProtocolVersionSupported",
  "isRecord",
  "parseEnvelope",
];

describe("index（导出面契约与跨模块不变量）", () => {
  it("导出面锁定：36 个运行时导出精确定义（增删必须同步本清单）", () => {
    expect(Object.keys(protocol).sort()).toEqual([...EXPECTED_EXPORTS].sort());
    expect(EXPECTED_EXPORTS).toHaveLength(36);
  });

  it("关键导出形态", () => {
    expect(typeof protocol.parseEnvelope).toBe("function");
    expect(Array.isArray(protocol.MUSUME_STATES)).toBe(true);
    expect(protocol.LEASE_EXEMPT_STATES).toBeInstanceOf(Set);
    expect(typeof protocol.PRIORITY).toBe("object");
  });

  it("不变量 A：8 态（连字符）↔ 8 动作键（下划线）归一化一致", () => {
    const normalized = new Set(protocol.MUSUME_STATES.map((s) => s.replace(/-/g, "_")));
    expect(normalized).toEqual(new Set(protocol.ACTION_KEYS));
  });

  it("不变量 B：notify 四映射的枚举值齐备（R1：官方枚举漂移时先炸）", () => {
    const mapped = ["permission_prompt", "agent_needs_input", "idle_prompt", "agent_completed"];
    for (const v of mapped) expect(protocol.NOTIFICATION_TYPES).toContain(v);
    for (const k of [
      "notify.permission",
      "notify.needs_input",
      "notify.idle",
      "notify.completed",
    ] as const) {
      expect(protocol.CANON_KINDS).toContain(k);
    }
  });
});
