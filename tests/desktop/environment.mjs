export const systemMotion = process.env.CODE_MUSUME_UI_SYSTEM_MOTION ?? "no-preference";
if (!["reduce", "no-preference"].includes(systemMotion))
  throw new Error("Unsupported desktop-test system motion preference");

// These switches affect only the isolated test WebView, not Windows settings.
export const browserArguments = [
  "--force-device-scale-factor=1",
  "--force-color-profile=srgb",
  "--disable-lcd-text",
  systemMotion === "reduce"
    ? "--force-prefers-reduced-motion"
    : "--force-prefers-no-reduced-motion",
].join(" ");
