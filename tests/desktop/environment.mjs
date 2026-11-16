import { createHash } from "node:crypto";

export const systemMotion = process.env.CODE_MUSUME_UI_SYSTEM_MOTION ?? "no-preference";
if (!["reduce", "no-preference"].includes(systemMotion))
  throw new Error("Unsupported desktop-test system motion preference");

// The e2e Rust host applies the preference and profile directory via native WebView
// options: elevated Windows hosts ignore WEBVIEW2_* environment overrides.
export const codexTestPipe = `\\\\.\\pipe\\code-musume-e2e-${createHash("sha256")
  .update(process.env.CODE_MUSUME_UI_RUN_DIR ?? "")
  .digest("hex")
  .slice(0, 24)}`;
