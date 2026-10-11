import { describe, expect, it } from "vitest";
import {
  ENVELOPE_MAX_PAYLOAD_BYTES,
  isProtocolVersionSupported,
  PROTOCOL_VERSION,
  parseEnvelope,
} from "../src/index";

/** 合法信封工厂（每次返回新对象，避免测试间共享引用）。 */
function goodEnvelope() {
  return {
    v: PROTOCOL_VERSION,
    agent: "claude-code",
    event: "PreToolUse",
    ts: 1770000000000,
    src: { cli: "0.0.0", pid: 1234 },
    payload: '{"hook_event_name":"PreToolUse"}',
  };
}

describe("parseEnvelope（事件协议 §3）", () => {
  describe("正常路径", () => {
    it("合法信封逐字段保留", () => {
      const env = goodEnvelope();
      expect(parseEnvelope(env)).toEqual(env);
    });

    it("JSON 往返不变（HTTP 传输的序列化不变量）", () => {
      const env = goodEnvelope();
      expect(parseEnvelope(JSON.parse(JSON.stringify(env)))).toEqual(env);
    });

    it("未知字段被丢弃（协议 §2：未知字段忽略），键集精确", () => {
      const out = parseEnvelope({
        ...goodEnvelope(),
        extra: 1,
        src: { cli: "0.0.0", pid: 1, extra: 2 },
      });
      if (!out) throw new Error("期望非 null");
      expect(Object.keys(out)).toEqual(["v", "agent", "event", "ts", "src", "payload"]);
      expect(Object.keys(out.src)).toEqual(["cli", "pid"]);
    });

    it("不修改入参（严格模式下写冻结对象会抛异常）", () => {
      const env = Object.freeze({
        ...goodEnvelope(),
        src: Object.freeze({ cli: "0.0.0", pid: 1 }),
      });
      expect(() => parseEnvelope(env)).not.toThrow();
    });

    it("v 不做版本校验（分层契约：版本判断归调用方 ingest）", () => {
      expect(parseEnvelope({ ...goodEnvelope(), v: 2 })).not.toBeNull();
      expect(isProtocolVersionSupported(2)).toBe(false);
    });
  });

  describe("边界", () => {
    it("空 payload 合法（协议 §5.3 降级路径：不得在信封层整条丢弃）", () => {
      const out = parseEnvelope({ ...goodEnvelope(), payload: "" });
      expect(out).not.toBeNull();
      expect(out?.payload).toBe("");
    });

    it("payload 超 64KB 照收（截断 / parseError 归 CLI 侧，Phase 2）", () => {
      const big = "a".repeat(ENVELOPE_MAX_PAYLOAD_BYTES + 1);
      expect(parseEnvelope({ ...goodEnvelope(), payload: big })).not.toBeNull();
    });

    it("ts：0 / 负数 / 非整数均接受（只要求有限数——宽松行为显式记录）", () => {
      for (const ts of [0, -1, 1.5]) {
        expect(parseEnvelope({ ...goodEnvelope(), ts })).not.toBeNull();
      }
    });

    it("v / src.pid 的非整数同样接受（结构校验与业务校验分层）", () => {
      expect(parseEnvelope({ ...goodEnvelope(), v: 1.5 })).not.toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), src: { cli: "0.0.0", pid: 1.5 } })).not.toBeNull();
    });

    it.each(["read", "size", "json"] as const)("parseError.kind=%s 保留", (kind) => {
      const out = parseEnvelope({ ...goodEnvelope(), parseError: { kind } });
      expect(out?.parseError).toEqual({ kind });
    });

    it("非法 kind 的 parseError 被静默丢弃、不整体拒绝（重要契约）", () => {
      const out = parseEnvelope({ ...goodEnvelope(), parseError: { kind: "other" } });
      expect(out).not.toBeNull();
      expect(out?.parseError).toBeUndefined();
    });

    it("parseError 非对象（字符串/数组）→ 丢弃", () => {
      expect(parseEnvelope({ ...goodEnvelope(), parseError: "oops" })?.parseError).toBeUndefined();
      expect(parseEnvelope({ ...goodEnvelope(), parseError: [] })?.parseError).toBeUndefined();
    });

    it("parseError.detail：非字符串丢弃 / 空串保留", () => {
      const dropped = parseEnvelope({
        ...goodEnvelope(),
        parseError: { kind: "json", detail: 123 },
      });
      expect(dropped?.parseError).toEqual({ kind: "json" });
      const kept = parseEnvelope({ ...goodEnvelope(), parseError: { kind: "json", detail: "" } });
      expect(kept?.parseError).toEqual({ kind: "json", detail: "" });
    });

    it("ENVELOPE_MAX_PAYLOAD_BYTES = 64 KiB", () => {
      expect(ENVELOPE_MAX_PAYLOAD_BYTES).toBe(64 * 1024);
    });
  });

  describe("异常输入（永不抛异常，一律 null）", () => {
    it.each([null, undefined, "string", 1, true, [], [1]])("顶层非对象", (v) => {
      expect(parseEnvelope(v)).toBeNull();
    });

    it.each(["v", "agent", "event", "ts", "src", "payload"] as const)("缺字段 %s → null", (key) => {
      const env: Record<string, unknown> = { ...goodEnvelope() };
      delete env[key];
      expect(parseEnvelope(env)).toBeNull();
    });

    it("空串：agent / event / src.cli → null（payload 空串例外，见「边界」）", () => {
      expect(parseEnvelope({ ...goodEnvelope(), agent: "" })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), event: "" })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), src: { cli: "", pid: 1 } })).toBeNull();
    });

    it("类型错误 → null", () => {
      expect(parseEnvelope({ ...goodEnvelope(), v: "1" })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), ts: "1" })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), agent: 1 })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), event: null })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), src: null })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), src: "x" })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), src: { cli: "0.0.0", pid: "1234" } })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), payload: null })).toBeNull();
    });

    it("NaN / Infinity → null（isFiniteNumber 把关）", () => {
      expect(parseEnvelope({ ...goodEnvelope(), v: Number.NaN })).toBeNull();
      expect(parseEnvelope({ ...goodEnvelope(), ts: Number.POSITIVE_INFINITY })).toBeNull();
    });

    it("对全部畸形输入永不抛异常", () => {
      const malformed: unknown[] = [null, undefined, "s", 1, [], {}, { v: {} }, { payload: [] }];
      for (const x of malformed) {
        expect(() => parseEnvelope(x)).not.toThrow();
      }
    });
  });
});
