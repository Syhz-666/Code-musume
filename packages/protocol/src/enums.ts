/** 官方枚举取值（保留原文大小写；均带 unknown 兜底）——`docs\事件协议-2026-10-11.md` §4.4。 */

export const SESSION_SOURCES = ["startup", "resume", "clear", "compact", "fork"] as const;
export type SessionSource = (typeof SESSION_SOURCES)[number] | "unknown";

export const SESSION_END_REASONS = [
  "clear",
  "resume",
  "logout",
  "prompt_input_exit",
  "other",
] as const;
export type SessionEndReason = (typeof SESSION_END_REASONS)[number] | "unknown";

/** 12 值（2026-10-11 复核）；本项目仅映射 4 个，其余显式忽略（状态机规范 §7.3）。 */
export const NOTIFICATION_TYPES = [
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
] as const;
export type NotificationType = (typeof NOTIFICATION_TYPES)[number] | "unknown";

/** 12 值（官方含 unknown）——状态机规范 §7.4。 */
export const STOP_ERRORS = [
  "rate_limit",
  "overloaded",
  "authentication_failed",
  "oauth_org_not_allowed",
  "account_on_hold",
  "billing_error",
  "invalid_request",
  "model_not_found",
  "server_error",
  "max_output_tokens",
  "cloud_credential_error",
  "unknown",
] as const;
export type StopError = (typeof STOP_ERRORS)[number];

export const PERMISSION_MODES = [
  "default",
  "plan",
  "acceptEdits",
  "auto",
  "dontAsk",
  "bypassPermissions",
] as const;
export type PermissionMode = (typeof PERMISSION_MODES)[number];

function asOneOf<T extends string>(list: readonly T[], fallback: T) {
  return (v: unknown): T =>
    typeof v === "string" && (list as readonly string[]).includes(v) ? (v as T) : fallback;
}

export const asSessionSource = asOneOf<SessionSource>([...SESSION_SOURCES, "unknown"], "unknown");
export const asSessionEndReason = asOneOf<SessionEndReason>(
  [...SESSION_END_REASONS, "unknown"],
  "unknown",
);
export const asNotificationType = asOneOf<NotificationType>(
  [...NOTIFICATION_TYPES, "unknown"],
  "unknown",
);
export const asStopError = asOneOf<StopError>([...STOP_ERRORS], "unknown");
export const asPermissionMode = (v: unknown): PermissionMode | undefined =>
  typeof v === "string" && (PERMISSION_MODES as readonly string[]).includes(v)
    ? (v as PermissionMode)
    : undefined;
