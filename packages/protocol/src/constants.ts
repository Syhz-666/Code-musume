/**
 * 全部数值的唯一真相源：`docs\状态机规范-2026-10-11.md` §5。
 * 改动顺序：先改规范，再改这里（R16 防漂移）。
 */

export const LEASE_TTL_MS = 300_000;
export const DONE_TTL_MS = 8_000;
export const NOTIFICATION_TTL_MS = 20_000;
export const ERROR_TTL_MS = 60_000;
export const MIN_DWELL_MS = 4_000;
export const DEDUPE_WINDOW_MS = 3_000;
export const SLEEPING_AFTER_IDLE_MS = 600_000;
export const STALE_AFTER_MS = 600_000;
export const INGEST_COALESCE_MS = 200;
export const MAX_SESSIONS = 8;
export const MIN_FORM_DWELL_MS = 5_000;

/** 8 态状态 id。 */
export const MUSUME_STATES = [
  "waiting-permission",
  "error",
  "notification",
  "done",
  "working",
  "thinking",
  "idle",
  "sleeping",
] as const;
export type MusumeState = (typeof MUSUME_STATES)[number];

/** 优先级（数值冻结于状态机规范 §5；done=80 为暂定值，A9 M2 复审）。 */
export const PRIORITY: Record<MusumeState, number> = {
  "waiting-permission": 100,
  error: 90,
  notification: 85,
  done: 80,
  working: 60,
  thinking: 50,
  idle: 30,
  sleeping: 10,
};

/** 豁免租约回收的状态（状态机规范 §4）。 */
export const LEASE_EXEMPT_STATES: ReadonlySet<MusumeState> = new Set(["waiting-permission"]);

/** 瞬态状态（超 TTL 回落 idle）。 */
export const TRANSIENT_TTL_MS: Partial<Record<MusumeState, number>> = {
  done: DONE_TTL_MS,
  notification: NOTIFICATION_TTL_MS,
  error: ERROR_TTL_MS,
};
