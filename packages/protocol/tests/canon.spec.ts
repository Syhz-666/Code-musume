import { describe, expect, it } from "vitest";
import { CANON_KINDS, type CanonEvent } from "../src/index";

describe("canon（事件协议 §4.3；CanonKind 封闭枚举）", () => {
  it("CANON_KINDS：精确 18 值且有序、无重复", () => {
    expect(CANON_KINDS).toEqual([
      "session.start",
      "session.end",
      "prompt.submit",
      "tool.activity",
      "tool.failure",
      "permission.request",
      "permission.denied",
      "notify.permission",
      "notify.needs_input",
      "notify.idle",
      "notify.completed",
      "turn.end",
      "turn.fail",
      "subagent.start",
      "subagent.stop",
      "compact.start",
      "compact.end",
      "unknown",
    ]);
    expect(new Set(CANON_KINDS).size).toBe(18);
  });

  const base = { agentId: "claude-code", sessionId: "s1", at: 1 };

  it("18 个变体全覆盖：接口与枚举一一对应（类型级 + 运行时双守护）", () => {
    const allEvents: CanonEvent[] = [
      { ...base, kind: "session.start", source: "startup" },
      { ...base, kind: "session.end", reason: "clear" },
      { ...base, kind: "prompt.submit" },
      { ...base, kind: "tool.activity", tool: { name: "Read", useId: "t1" }, durationMs: 12 },
      { ...base, kind: "tool.failure", error: "boom", isInterrupt: false },
      { ...base, kind: "permission.request", tool: { name: "Bash" } },
      { ...base, kind: "permission.denied", reason: "auto" },
      { ...base, kind: "notify.permission", notificationType: "permission_prompt" },
      { ...base, kind: "notify.needs_input", notificationType: "agent_needs_input" },
      { ...base, kind: "notify.idle", notificationType: "idle_prompt" },
      { ...base, kind: "notify.completed", notificationType: "agent_completed" },
      { ...base, kind: "turn.end" },
      { ...base, kind: "turn.fail", error: "rate_limit" },
      { ...base, kind: "subagent.start", subagentType: "explore" },
      { ...base, kind: "subagent.stop" },
      { ...base, kind: "compact.start", trigger: "auto" },
      { ...base, kind: "compact.end", trigger: "manual" },
      { ...base, kind: "unknown", rawEvent: "SomeNewEvent" },
    ];
    expect(new Set(allEvents.map((e) => e.kind))).toEqual(new Set(CANON_KINDS));
  });

  it("可选字段可整体省略（最简构造仍为合法 CanonEvent）", () => {
    const ev: CanonEvent = { ...base, kind: "tool.activity" };
    expect(ev.kind).toBe("tool.activity");
  });

  it("unknown.rawEvent 必填（@ts-expect-error 锁定：若被改成可选，typecheck 直接失败）", () => {
    // @ts-expect-error rawEvent 是必填字段，缺失必须编译失败
    const bad: CanonEvent = { ...base, kind: "unknown" };
    expect(bad).toBeDefined();
  });

  it("4 个 notify kind 齐备", () => {
    for (const k of [
      "notify.permission",
      "notify.needs_input",
      "notify.idle",
      "notify.completed",
    ] as const) {
      expect(CANON_KINDS).toContain(k);
    }
  });
});
