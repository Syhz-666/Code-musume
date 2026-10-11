import { describe, expect, it } from "vitest";
import { PROTOCOL_VERSION, parseEnvelope } from "../src/index";

describe("parseEnvelope（事件协议 §3 冒烟）", () => {
  it("接受合法信封并原样保留字段", () => {
    const env = {
      v: PROTOCOL_VERSION,
      agent: "claude-code",
      event: "PreToolUse",
      ts: 1770000000000,
      src: { cli: "0.0.0", pid: 1234 },
      payload: '{"hook_event_name":"PreToolUse"}',
    };
    expect(parseEnvelope(env)).toEqual(env);
  });

  it("拒绝缺失必填字段与非法输入", () => {
    expect(parseEnvelope(null)).toBeNull();
    expect(parseEnvelope("string")).toBeNull();
    expect(parseEnvelope({ v: 1 })).toBeNull();
    expect(
      parseEnvelope({ v: 1, agent: "a", event: "e", ts: 1, src: { cli: "x" }, payload: "" }),
    ).toBeNull();
  });

  it("保留 parseError 标记（畸形 payload 仍发送）", () => {
    const env = parseEnvelope({
      v: 1,
      agent: "claude-code",
      event: "Stop",
      ts: 1,
      src: { cli: "0.0.0", pid: 1 },
      payload: "not-json",
      parseError: { kind: "json" },
    });
    expect(env?.parseError).toEqual({ kind: "json" });
  });
});
