import { isFiniteNumber, isNonEmptyString, isRecord } from "./guard";
import { PROTOCOL_VERSION } from "./version";

/** Envelope 解析错误标记（事件协议 §3）。 */
export interface EnvelopeParseError {
  kind: "read" | "size" | "json";
  detail?: string;
}

export interface EnvelopeSrc {
  cli: string;
  pid: number;
}

/** hook CLI → 桌宠进程 的线协议信封（`docs\事件协议-2026-10-11.md` §3，唯一真相源）。 */
export interface Envelope {
  /** 协议版本（=1）。 */
  v: number;
  /** agent 标识（来自 argv --agent，不解析 payload）。 */
  agent: string;
  /** 官方事件名原文（来自 argv --event）。 */
  event: string;
  /** UTC epoch ms，CLI 生成。 */
  ts: number;
  src: EnvelopeSrc;
  /** stdin 原文（不解析、不重编码）。R14：仅内存使用，不落盘。 */
  payload: string;
  parseError?: EnvelopeParseError;
}

/** 信封大小上限（截断 + parseError.kind="size"，仍发送）。 */
export const ENVELOPE_MAX_PAYLOAD_BYTES = 64 * 1024;

/** 校验并解析信封；不合法返回 null（调用方按「畸形」处理，不抛异常）。 */
export function parseEnvelope(input: unknown): Envelope | null {
  if (!isRecord(input)) return null;
  if (!isFiniteNumber(input.v)) return null;
  if (!isNonEmptyString(input.agent) || !isNonEmptyString(input.event)) return null;
  if (!isFiniteNumber(input.ts) || !isNonEmptyString(input.payload)) return null;
  const src = input.src;
  if (!isRecord(src) || !isNonEmptyString(src.cli) || !isFiniteNumber(src.pid)) return null;

  const env: Envelope = {
    v: input.v,
    agent: input.agent,
    event: input.event,
    ts: input.ts,
    src: { cli: src.cli, pid: src.pid },
    payload: input.payload,
  };
  const pe = input.parseError;
  if (isRecord(pe) && (pe.kind === "read" || pe.kind === "size" || pe.kind === "json")) {
    env.parseError =
      typeof pe.detail === "string" ? { kind: pe.kind, detail: pe.detail } : { kind: pe.kind };
  }
  return env;
}

export function isProtocolVersionSupported(v: number): boolean {
  return v === PROTOCOL_VERSION;
}
