/** RenderPlan（core → 渲染层）——`docs\事件协议-2026-10-11.md` §6。 */

export const ACTION_KEYS = [
  "idle",
  "thinking",
  "working",
  "waiting_permission",
  "notification",
  "done",
  "error",
  "sleeping",
] as const;
export type ActionKey = (typeof ACTION_KEYS)[number];

/** 气泡键：只允许状态文案 key——不得承载 prompt / 代码 / 路径 / last_assistant_message（R14）。 */
export const BUBBLE_KEYS = ["waiting_permission", "notification", "done", "error"] as const;
export type BubbleKey = (typeof BUBBLE_KEYS)[number];

export interface SlotSpec {
  slot: string;
  asset?: string;
}

export interface RenderPlan {
  animation: ActionKey;
  form: string;
  slots: SlotSpec[];
  bubble?: {
    key: BubbleKey;
    params?: Record<string, string>;
    ttlMs?: number;
  };
  transition?: {
    durationMs: number;
    easing: "linear" | "easeInOut";
  };
  /** 0..1，表达强度（waiting-permission > notification > done 的三级分层）。 */
  intensity: number;
  /** 音效键（M1 不实现，字段先留）。 */
  sfx?: string;
}

export const DEFAULT_RENDER_PLAN: RenderPlan = {
  animation: "idle",
  form: "default",
  slots: [],
  intensity: 0.3,
};
