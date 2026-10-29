import type { Clip } from "@code-musume/protocol";

export type AnimationPhase = "enter" | "loop" | "exit";

/** Presentation-only sequencing. Agent state and preview state stay outside it. */
export class SpritePlayback {
  private state = "";
  private requested = "";
  private phase: AnimationPhase = "loop";
  private since = 0;
  private reverseFrom: number | null = null;
  private reverseDuration = 0;
  private generation = 0;

  reset() {
    this.state = "";
    this.requested = "";
    this.generation++;
  }

  private begin(state: string, now: number, clips: Record<string, Clip>) {
    this.state = state;
    this.since = now;
    this.phase = clips[`${state}-enter`] ? "enter" : "loop";
    this.reverseFrom = null;
    this.generation++;
  }

  sample(clips: Record<string, Clip>, requested: string, now: number, animate: boolean) {
    if (!animate) {
      if (this.state !== requested || this.phase !== "loop") this.generation++;
      this.state = this.requested = requested;
      this.phase = "loop";
      this.since = now;
      this.reverseFrom = null;
    } else if (!this.state) {
      this.requested = requested;
      this.begin(requested, now, clips);
    } else if (requested !== this.requested) {
      this.requested = requested;
      // A pending exit always leads to the latest request, never a queue of old states.
      if (this.phase !== "exit" && this.state !== requested) {
        const entry = clips[`${this.state}-enter`];
        if (this.phase === "enter" && entry) {
          this.reverseFrom = Math.min(
            entry.frames - 1,
            Math.floor(((now - this.since) * entry.fps) / 1000),
          );
          this.reverseDuration = Math.min(
            480,
            Math.max(100, (this.reverseFrom * 1000) / entry.fps),
          );
          this.phase = "exit";
          this.since = now;
          this.generation++;
        } else if (clips[`${this.state}-exit`]) {
          this.phase = "exit";
          this.reverseFrom = null;
          this.since = now;
          this.generation++;
        } else {
          this.begin(requested, now, clips);
        }
      }
    }
    for (let step = 0; animate && step < 3; step++) {
      const key =
        this.phase === "exit" && this.reverseFrom !== null
          ? `${this.state}-enter`
          : this.phase === "loop"
            ? this.state
            : `${this.state}-${this.phase}`;
      const clip = clips[key] ?? clips.idle;
      if (!clip || this.phase === "loop") break;
      const duration =
        this.reverseFrom !== null && this.phase === "exit"
          ? this.reverseDuration
          : (clip.frames * 1000) / clip.fps;
      if (now - this.since < duration) break;
      if (this.phase === "exit") this.begin(this.requested, this.since + duration, clips);
      else {
        this.since += duration;
        this.phase = "loop";
        this.generation++;
      }
    }
    const segment =
      this.phase === "exit" && this.reverseFrom !== null
        ? `${this.state}-enter`
        : this.phase === "loop"
          ? this.state
          : `${this.state}-${this.phase}`;
    const key = clips[segment] ? segment : "idle";
    const clip = clips[key];
    if (!clip) return null;
    const tick = Math.max(0, Math.floor(((now - this.since) * clip.fps) / 1000));
    let index = !animate ? 0 : clip.looping ? tick % clip.frames : Math.min(clip.frames - 1, tick);
    if (animate && this.phase === "exit" && this.reverseFrom !== null) {
      const t = Math.min(1, Math.max(0, (now - this.since) / this.reverseDuration));
      index = Math.round(this.reverseFrom * (1 - t * t * (3 - 2 * t)));
    }
    return {
      clip,
      key,
      index,
      phase: this.phase,
      state: this.state,
      token: `${this.generation}:${key}`,
      ready: this.state === requested && this.phase !== "exit",
      transitioning: this.phase !== "loop",
    };
  }
}
