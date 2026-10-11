import { describe, expect, it } from "vitest";
import { isProtocolVersionSupported, PROTOCOL_VERSION } from "../src/index";

describe("version（事件协议 §2）", () => {
  it("PROTOCOL_VERSION 冻结为 1——bump 协议版本时必须同步本测试", () => {
    expect(PROTOCOL_VERSION).toBe(1);
  });

  it("支持当前版本", () => {
    expect(isProtocolVersionSupported(PROTOCOL_VERSION)).toBe(true);
  });

  it.each([0, 2, -1, 1.5, 100])("拒绝其他版本：%s", (v) => {
    expect(isProtocolVersionSupported(v)).toBe(false);
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY])(
    "拒绝非有限数：%s",
    (v) => {
      expect(isProtocolVersionSupported(v)).toBe(false);
    },
  );
});
