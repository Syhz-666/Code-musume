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
  private presented: { token: string; index: number } | null = null;

  reset() {
    this.state = "";
    this.requested = "";
    this.presented = null;
    this.generation++;
  }

  markPresented(sample: { token: string; index: number }) {
    this.presented = { token: sample.token, index: sample.index };
  }

  private token(key: string) {
    return `${this.generation}:${key}`;
  }

  private showed(key: string, index: number) {
    return this.presented?.token === this.token(key) && this.presented.index === index;
  }

  private exitLead(sequenced: boolean) {
    // Join the held pose before the recorded lowering movement advances.
    return sequenced && this.phase === "exit" && this.reverseFrom === null ? 80 : 0;
  }

  private begin(state: string, now: number, clips: Record<string, Clip>) {
    this.state = state;
    this.since = now;
    this.phase = clips[`${state}-enter`] ? "enter" : "loop";
    this.reverseFrom = null;
    this.generation++;
  }

  sample(
    clips: Record<string, Clip>,
    requested: string,
    now: number,
    animate: boolean,
    sequenced = false,
  ) {
    if (!animate) {
      if (this.state !== requested || this.phase !== "loop") this.generation++;
      this.state = this.requested = requested;
      this.phase = "loop";
      this.since = now;
      this.reverseFrom = null;
    } else if (!this.state) {
      this.requested = requested;
      this.begin(requested, now, clips);
    } else {
      const changed = requested !== this.requested;
      this.requested = requested;
      const reaction = clips[this.state];
      const oneShot =
        sequenced &&
        this.phase !== "exit" &&
        (this.state === "notification" || this.state === "done") &&
        reaction &&
        !reaction.looping;
      const finishing =
        oneShot &&
        requested !== "waiting-permission" &&
        requested !== "error" &&
        (this.phase === "enter" ||
          now - this.since < (reaction.frames * 1000) / reaction.fps ||
          !this.showed(this.state, reaction.frames - 1));
      // A pending exit always leads to the latest request, never a queue of old states.
      if ((changed || oneShot) && !finishing && this.phase !== "exit" && this.state !== requested) {
        const entryKey = `${this.state}-enter`;
        const entry = clips[entryKey];
        if (this.phase === "enter" && entry) {
          this.reverseFrom = sequenced
            ? this.presented?.token === this.token(entryKey)
              ? this.presented.index
              : 0
            : Math.min(entry.frames - 1, Math.floor(((now - this.since) * entry.fps) / 1000));
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
          : (clip.frames * 1000) / clip.fps + this.exitLead(sequenced);
      if (now - this.since < duration) break;
      const lastFrame = this.phase === "exit" && this.reverseFrom !== null ? 0 : clip.frames - 1;
      if (sequenced && !this.showed(key, lastFrame)) break;
      const nextStart = sequenced ? now : this.since + duration;
      if (this.phase === "exit") this.begin(this.requested, nextStart, clips);
      else {
        this.since = nextStart;
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
    const still = `${requested}-still`;
    const key = !animate && clips[still] ? still : clips[segment] ? segment : "idle";
    const clip = clips[key];
    if (!clip) return null;
    const tick = Math.max(
      0,
      Math.floor(((now - this.since - this.exitLead(sequenced)) * clip.fps) / 1000),
    );
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
      token: this.token(key),
      ready: this.state === requested && this.phase !== "exit",
      transitioning:
        this.phase !== "loop" || (animate && sequenced && !clip.looping && index < clip.frames - 1),
    };
  }
}
