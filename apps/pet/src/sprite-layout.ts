import type { Clip } from "@code-musume/protocol";

/** Clips advance across columns, then continue on the next atlas row. */
export function spriteFramePosition(
  columns: number,
  clip: Pick<Clip, "row" | "column">,
  index: number,
) {
  const absolute = clip.row * columns + (clip.column ?? 0) + index;
  return { column: absolute % columns, row: Math.floor(absolute / columns) };
}
