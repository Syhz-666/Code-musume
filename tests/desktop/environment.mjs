export const systemMotion = process.env.CODE_MUSUME_UI_SYSTEM_MOTION ?? "no-preference";
if (!["reduce", "no-preference"].includes(systemMotion))
  throw new Error("Unsupported desktop-test system motion preference");

// The e2e Rust host applies the preference and profile directory via native WebView
// options: elevated Windows hosts ignore WEBVIEW2_* environment overrides.
