import { type DisplayState, PROTOCOL_VERSION, type Projection } from "./generated";
export const STATE_LABELS: Record<DisplayState, string> = {
  "waiting-permission": "等你处理",
  error: "这轮遇到问题",
  done: "这轮好了",
  notification: "有个提醒",
  working: "干活中",
  thinking: "思考中",
  idle: "陪着你",
  sleeping: "休息一会儿",
};
/** Revision filtering only; work semantics and timers belong to Rust. */
export class ProjectionStore {
  current: Projection | null = null;
  private retired = new Set<string>();
  accept(next: Projection): boolean {
    if (
      next.v !== PROTOCOL_VERSION ||
      !Number.isSafeInteger(next.revision) ||
      next.revision < 0 ||
      !next.instanceId ||
      this.retired.has(next.instanceId)
    )
      return false;
    if (this.current?.instanceId === next.instanceId && next.revision <= this.current.revision)
      return false;
    if (this.current && this.current.instanceId !== next.instanceId) {
      this.retired.add(this.current.instanceId);
      if (this.retired.size > 16) this.retired.delete(this.retired.values().next().value ?? "");
    }
    this.current = next;
    return true;
  }
}
