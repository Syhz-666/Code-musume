import { describe, expect, it } from "vitest";
import { isFiniteNumber, isNonEmptyString, isRecord } from "../src/index";

describe("guard（公共解析底座）", () => {
  describe("isRecord", () => {
    it.each([{}, { a: 1 }, Object.create(null)])("接受普通对象", (v) => {
      expect(isRecord(v)).toBe(true);
    });

    it("明确拒绝数组（parseEnvelope 依赖这一点）", () => {
      expect(isRecord([])).toBe(false);
      expect(isRecord([1, 2])).toBe(false);
    });

    it.each([null, undefined, "s", 1, true])("拒绝非对象值", (v) => {
      expect(isRecord(v)).toBe(false);
    });

    it("已知宽松行为：只做 typeof 判断，Date/Map 也会通过（显式记录，勿依赖）", () => {
      expect(isRecord(new Date())).toBe(true);
      expect(isRecord(new Map())).toBe(true);
    });
  });

  describe("isNonEmptyString", () => {
    it.each(["a", "0", " ", "\n"])("接受非空字符串", (v) => {
      expect(isNonEmptyString(v)).toBe(true);
    });

    it("空串拒绝；空白串算非空（显式记录）", () => {
      expect(isNonEmptyString("")).toBe(false);
      expect(isNonEmptyString(" ")).toBe(true);
    });

    it.each([null, undefined, 0, [], {}])("拒绝非字符串", (v) => {
      expect(isNonEmptyString(v)).toBe(false);
    });
  });

  describe("isFiniteNumber", () => {
    it.each([0, -1, 1.5, Number.MAX_SAFE_INTEGER])("接受有限数（不校验整数）", (v) => {
      expect(isFiniteNumber(v)).toBe(true);
    });

    it.each([Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY])(
      "拒绝非有限数：%s",
      (v) => {
        expect(isFiniteNumber(v)).toBe(false);
      },
    );

    it.each(["1", null, undefined, []])("拒绝非 number", (v) => {
      expect(isFiniteNumber(v)).toBe(false);
    });
  });
});
