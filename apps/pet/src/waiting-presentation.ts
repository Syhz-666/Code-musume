import type { DisplayState } from "@code-musume/protocol";

type Pose = {
  characterId: string;
  formId: string;
  state: DisplayState;
};

const WAITING_SETTLE_MS = 240;
const WAITING = "waiting-permission";

// Hooks and Desktop sync can update between two paints. Keep a brief change
// around an approval out of the animator while the bubble uses the latest state.
export class WaitingPresentation {
  private pose: Pose | undefined;
  private pending: { formId: string; waiting: boolean; since: number } | undefined;

  resolve(next: Pose, now: number, immediate: boolean): Pose {
    const current = this.pose;
    if (!immediate && current?.characterId === next.characterId) {
      if (current.state === next.state && current.formId === next.formId) {
        this.pending = undefined;
        return current;
      }
      const waiting = next.state === WAITING;
      const needsSettle =
        (current.state === WAITING && next.state !== "error") ||
        (waiting && next.formId !== current.formId);
      if (needsSettle) {
        if (this.pending?.formId !== next.formId || this.pending.waiting !== waiting) {
          this.pending = { formId: next.formId, waiting, since: now };
        }
        if (now - this.pending.since < WAITING_SETTLE_MS) return current;
      }
    }
    this.pending = undefined;
    this.pose = next;
    return next;
  }
}

// If the source outfit is requested again mid-change, retrace the frames
// already presented instead of showing the obsolete destination outfit.
export class CostumePlayback {
  private direction = 1;
  private origin = 0;
  private presented: number | undefined;

  constructor(
    private started: number,
    private readonly frames: number,
    private readonly fps: number,
  ) {}

  sample(now: number, returning: boolean) {
    const direction = returning ? -1 : 1;
    if (direction !== this.direction) {
      this.direction = direction;
      this.origin = this.presented ?? 0;
      this.started = now;
    }
    const raw = this.origin + direction * Math.floor(((now - this.started) * this.fps) / 1000);
    return {
      index: Math.max(0, Math.min(this.frames - 1, raw)),
      done: returning
        ? raw < 0 && this.presented === 0
        : raw >= this.frames && this.presented === this.frames - 1,
    };
  }

  markPresented(index: number) {
    this.presented = index;
  }
}
