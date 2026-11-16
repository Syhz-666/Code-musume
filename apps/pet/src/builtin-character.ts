import type { CharacterManifest, Clip } from "@code-musume/protocol";
import attentionUrl from "../assets/builtin/attention.png";
import codexIdleUrl from "../assets/builtin/codex-idle.png";
import codexReactionsUrl from "../assets/builtin/codex-reactions.webp";
import codexRestUrl from "../assets/builtin/codex-rest.webp";
import codexThinkingUrl from "../assets/builtin/codex-thinking.webp";
import codexWorkUrl from "../assets/builtin/codex-work.webp";
import costumeChangeUrl from "../assets/builtin/costume-change.webp";
import errorUrl from "../assets/builtin/error.png";
import manifest from "../assets/builtin/musume.json";
import sleepingUrl from "../assets/builtin/sleeping.png";
import spriteUrl from "../assets/builtin/spritesheet.png";
import thinkingUrl from "../assets/builtin/thinking.png";
import workingUrl from "../assets/builtin/working.png";

export const builtinManifest: CharacterManifest = {
  ...manifest,
  forms: manifest.forms.map(({ clips, ...form }) => {
    const defined: Record<string, Clip> = {};
    for (const [name, clip] of Object.entries(clips)) {
      if (clip) defined[name] = clip;
    }
    return { ...form, clips: defined };
  }),
};
export const builtinSpriteUrl = spriteUrl;
export function builtinAssetUrl(image: string) {
  if (image === "spritesheet.png") return spriteUrl;
  if (image === "thinking.png") return thinkingUrl;
  if (image === "working.png") return workingUrl;
  if (image === "attention.png") return attentionUrl;
  if (image === "codex-idle.png") return codexIdleUrl;
  if (image === "codex-reactions.webp") return codexReactionsUrl;
  if (image === "codex-rest.webp") return codexRestUrl;
  if (image === "codex-thinking.webp") return codexThinkingUrl;
  if (image === "codex-work.webp") return codexWorkUrl;
  if (image === "costume-change.webp") return costumeChangeUrl;
  if (image === "error.png") return errorUrl;
  if (image === "sleeping.png") return sleepingUrl;
  throw new Error("Unknown builtin atlas");
}
