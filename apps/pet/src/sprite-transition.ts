import type { CharacterForm, Clip } from "@code-musume/protocol";
import { spriteFramePosition } from "./sprite-layout";

/** Brief premultiplied blend between poses, shared by Pixi and the DOM renderer. */
export class SpriteTransition {
  readonly canvas = document.createElement("canvas");
  private snapshot = document.createElement("canvas");
  private key = "";
  private since = 0;
  active = false;

  constructor(stage: HTMLElement) {
    this.canvas.className = "sprite-transition";
    this.canvas.setAttribute("aria-hidden", "true");
    stage.appendChild(this.canvas);
  }

  reset() {
    this.key = "";
    this.active = false;
    this.canvas.style.display = "none";
  }

  paint(
    image: HTMLImageElement,
    form: CharacterForm,
    clip: Clip,
    index: number,
    key: string,
    now: number,
    animate: boolean,
    renderer: string,
  ) {
    if (this.canvas.width !== form.cellWidth || this.canvas.height !== form.cellHeight) {
      this.canvas.width = this.snapshot.width = form.cellWidth;
      this.canvas.height = this.snapshot.height = form.cellHeight;
      this.reset();
    }
    if (animate && this.key && key !== this.key) {
      const snapshot = this.snapshot.getContext("2d");
      snapshot?.clearRect(0, 0, form.cellWidth, form.cellHeight);
      // Freeze the already mixed pose when a transition is interrupted.
      snapshot?.drawImage(this.canvas, 0, 0);
      this.since = now;
      this.active = true;
    }
    if (!animate) this.active = false;
    this.key = key;
    const context = this.canvas.getContext("2d");
    if (!context) return;
    const position = spriteFramePosition(form.columns, clip, index);
    const progress = Math.min(1, Math.max(0, (now - this.since) / 180));
    const weight = this.active ? progress * progress * (3 - 2 * progress) : 1;
    context.clearRect(0, 0, form.cellWidth, form.cellHeight);
    context.globalCompositeOperation = "source-over";
    context.globalAlpha = 1 - weight;
    if (this.active) context.drawImage(this.snapshot, 0, 0);
    context.globalCompositeOperation = "lighter";
    context.globalAlpha = weight;
    context.drawImage(
      image,
      position.column * form.cellWidth,
      position.row * form.cellHeight,
      form.cellWidth,
      form.cellHeight,
      0,
      0,
      form.cellWidth,
      form.cellHeight,
    );
    context.globalCompositeOperation = "source-over";
    context.globalAlpha = 1;
    if (progress >= 1) this.active = false;
    const height = (144 * form.cellHeight) / form.cellWidth;
    this.canvas.style.width = "144px";
    this.canvas.style.height = `${height}px`;
    this.canvas.style.top = `${renderer === "dom" ? 7 : 83 - height / 2}px`;
    this.canvas.style.display = this.active ? "block" : "none";
  }
}
