import { describe, expect, it } from "vitest";
import {
  asNotificationType,
  asPermissionMode,
  asSessionEndReason,
  asSessionSource,
  asStopError,
  NOTIFICATION_TYPES,
  PERMISSION_MODES,
  SESSION_END_REASONS,
  SESSION_SOURCES,
  STOP_ERRORS,
} from "../src/index";

describe("enums（事件协议 §4.4；官方枚举漂移时此处先炸 —— R1 第一道防线）", () => {
  describe("值集精确锁定", () => {
    it("SESSION_SOURCES：5 值有序、不含 unknown（兜底只在类型里）", () => {
      expect(SESSION_SOURCES).toEqual(["startup", "resume", "clear", "compact", "fork"]);
      expect(SESSION_SOURCES).toHaveLength(5);
    });

    it("SESSION_END_REASONS：不含已移除的 bypass_permissions_disabled", () => {
      expect(SESSION_END_REASONS).toEqual([
        "clear",
        "resume",
        "logout",
        "prompt_input_exit",
        "other",
      ]);
      expect(SESSION_END_REASONS).not.toContain("bypass_permissions_disabled");
    });

    it("NOTIFICATION_TYPES：12 值", () => {
      expect(NOTIFICATION_TYPES).toEqual([
        "permission_prompt",
        "idle_prompt",
        "auth_success",
        "elicitation_dialog",
        "elicitation_url_dialog",
        "elicitation_complete",
        "elicitation_response",
        "agent_needs_input",
        "agent_completed",
        "quota_auto_resume_fired",
        "quota_auto_resume_stale",
        "quota_auto_resume_disabled",
      ]);
      expect(NOTIFICATION_TYPES).toHaveLength(12);
    });

    it("STOP_ERRORS：12 值且含 unknown（官方枚举本身包含，与其他表语义不同）", () => {
      expect(STOP_ERRORS).toHaveLength(12);
      expect(STOP_ERRORS).toContain("unknown");
      expect(STOP_ERRORS).toContain("rate_limit");
      expect(STOP_ERRORS).toContain("max_output_tokens");
    });

    it("PERMISSION_MODES：不含 manual（手动模式一律传 default）", () => {
      expect(PERMISSION_MODES).toEqual([
        "default",
        "plan",
        "acceptEdits",
        "auto",
        "dontAsk",
        "bypassPermissions",
      ]);
      expect(PERMISSION_MODES).not.toContain("manual");
    });

    it("全部值集无重复", () => {
      for (const table of [
        SESSION_SOURCES,
        SESSION_END_REASONS,
        NOTIFICATION_TYPES,
        STOP_ERRORS,
        PERMISSION_MODES,
      ]) {
        expect(new Set(table).size).toBe(table.length);
      }
    });
  });

  describe("as* 兜底与大小写敏感性", () => {
    it("遍历性：表中每个值过一遍对应 as* 必回原值（防表与 as* 列表不同步）", () => {
      for (const v of SESSION_SOURCES) expect(asSessionSource(v)).toBe(v);
      for (const v of SESSION_END_REASONS) expect(asSessionEndReason(v)).toBe(v);
      for (const v of NOTIFICATION_TYPES) expect(asNotificationType(v)).toBe(v);
      for (const v of STOP_ERRORS) expect(asStopError(v)).toBe(v);
      for (const v of PERMISSION_MODES) expect(asPermissionMode(v)).toBe(v);
    });

    it("大小写敏感（协议 §4.4：保留官方原文大小写）", () => {
      expect(asSessionSource("Startup")).toBe("unknown");
      expect(asNotificationType("Permission_Prompt")).toBe("unknown");
      expect(asStopError("RATE_LIMIT")).toBe("unknown");
    });

    it("尾空格与空串 → unknown", () => {
      expect(asSessionSource("resume ")).toBe("unknown");
      expect(asSessionSource("")).toBe("unknown");
    });

    it("类型错误 → unknown", () => {
      expect(asSessionSource(null)).toBe("unknown");
      expect(asSessionSource(42)).toBe("unknown");
      expect(asSessionSource({})).toBe("unknown");
    });

    it("兜底值幂等；已移除值不再被识别", () => {
      expect(asSessionSource("unknown")).toBe("unknown");
      expect(asStopError("unknown")).toBe("unknown");
      expect(asSessionEndReason("bypass_permissions_disabled")).toBe("unknown");
      expect(asStopError(undefined)).toBe("unknown");
    });

    it("asPermissionMode 语义不同：返回 undefined（不是 unknown 字符串）", () => {
      expect(asPermissionMode("manual")).toBeUndefined();
      expect(asPermissionMode("unknown")).toBeUndefined();
      expect(asPermissionMode(undefined)).toBeUndefined();
      expect(asPermissionMode("default")).toBe("default");
    });
  });
});
