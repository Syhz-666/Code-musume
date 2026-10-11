import type {
  NotificationType,
  PermissionMode,
  SessionEndReason,
  SessionSource,
  StopError,
} from "./enums";
import type { Envelope } from "./envelope";

/** CanonKind 封闭枚举（`docs\事件协议-2026-10-11.md` §4.3）。 */
export const CANON_KINDS = [
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
] as const;
export type CanonKind = (typeof CANON_KINDS)[number];

export interface CanonToolRef {
  name: string;
  useId?: string;
}

interface CanonBase {
  agentId: string;
  sessionId: string;
  /** 事件时刻（UTC epoch ms）。 */
  at: number;
  tool?: CanonToolRef;
  /** 工具级耗时（不含权限框与 PreToolUse hook 耗时）。 */
  durationMs?: number;
  permissionMode?: PermissionMode;
  /** 仅内存使用（R14）；不进日志 / 诊断面板 / 气泡。 */
  cwd?: string;
}

export interface SessionStartCanon extends CanonBase {
  kind: "session.start";
  source: SessionSource;
  title?: string;
}
export interface SessionEndCanon extends CanonBase {
  kind: "session.end";
  reason: SessionEndReason;
}
export interface PromptSubmitCanon extends CanonBase {
  kind: "prompt.submit";
}
export interface ToolActivityCanon extends CanonBase {
  kind: "tool.activity";
}
export interface ToolFailureCanon extends CanonBase {
  kind: "tool.failure";
  error?: string;
  isInterrupt?: boolean;
}
export interface PermissionRequestCanon extends CanonBase {
  kind: "permission.request";
}
export interface PermissionDeniedCanon extends CanonBase {
  kind: "permission.denied";
  reason?: string;
}
export interface NotifyPermissionCanon extends CanonBase {
  kind: "notify.permission";
  notificationType: NotificationType;
}
export interface NotifyNeedsInputCanon extends CanonBase {
  kind: "notify.needs_input";
  notificationType: NotificationType;
}
export interface NotifyIdleCanon extends CanonBase {
  kind: "notify.idle";
  notificationType: NotificationType;
}
export interface NotifyCompletedCanon extends CanonBase {
  kind: "notify.completed";
  notificationType: NotificationType;
}
export interface TurnEndCanon extends CanonBase {
  kind: "turn.end";
}
export interface TurnFailCanon extends CanonBase {
  kind: "turn.fail";
  error: StopError;
}
export interface SubagentStartCanon extends CanonBase {
  kind: "subagent.start";
  subagentType?: string;
}
export interface SubagentStopCanon extends CanonBase {
  kind: "subagent.stop";
  subagentType?: string;
}
export interface CompactStartCanon extends CanonBase {
  kind: "compact.start";
  trigger?: "manual" | "auto";
}
export interface CompactEndCanon extends CanonBase {
  kind: "compact.end";
  trigger?: "manual" | "auto";
}
export interface UnknownCanon extends CanonBase {
  kind: "unknown";
  /** 透传原始事件名。 */
  rawEvent: string;
}

export type CanonEvent =
  | SessionStartCanon
  | SessionEndCanon
  | PromptSubmitCanon
  | ToolActivityCanon
  | ToolFailureCanon
  | PermissionRequestCanon
  | PermissionDeniedCanon
  | NotifyPermissionCanon
  | NotifyNeedsInputCanon
  | NotifyIdleCanon
  | NotifyCompletedCanon
  | TurnEndCanon
  | TurnFailCanon
  | SubagentStartCanon
  | SubagentStopCanon
  | CompactStartCanon
  | CompactEndCanon
  | UnknownCanon;

/** AgentAdapter 接口（事件协议 §5.1）。只做语义翻译：不判优先级、不决定形态、不接触时钟。 */
export interface AgentAdapter {
  id: string;
  matches(e: Envelope): boolean;
  /** 0..n：一个 hook 事件可拆成多个语义事件（Notification 按 type 拆分）。 */
  toCanon(e: Envelope): CanonEvent[];
  sessionKey(e: Envelope): string;
}
