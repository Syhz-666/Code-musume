import type { CharacterManifest } from "@code-musume/protocol";
import manifest from "../assets/builtin/musume.json";
import spriteUrl from "../assets/builtin/spritesheet.png";
import thinkingUrl from "../assets/builtin/thinking.png";
import workingUrl from "../assets/builtin/working.png";

export const builtinManifest: CharacterManifest = manifest;
export const builtinSpriteUrl = spriteUrl;
export function builtinAssetUrl(image: string) {
  if (image === "spritesheet.png") return spriteUrl;
  if (image === "thinking.png") return thinkingUrl;
  if (image === "working.png") return workingUrl;
  throw new Error("Unknown builtin atlas");
}
