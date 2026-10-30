import { writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  diagnosticHints,
  failureCode,
  readPipelineDiagnostics,
} from "../../tools/diagnostic-report.mjs";

export async function captureUiFailure(name, error) {
  const runDir = process.env.CODE_MUSUME_UI_RUN_DIR;
  const host = await readPipelineDiagnostics(join(runDir, "runtime/runtime.dev.json"));
  const windows = {};
  try {
    const original = await browser.getWindowHandle().catch(() => null);
    for (const label of await browser.getWindowHandles()) {
      try {
        await browser.switchToWindow(label);
        windows[label] = await browser.execute(async () => {
          const view = { ...document.querySelector("#app")?.dataset };
          const config = await window.__TAURI_INTERNALS__.invoke("get_config");
          return {
            view,
            preferences: {
              renderer: config.renderer,
              reducedMotion: config.reducedMotion,
              hidden: config.userHidden,
              fullscreenHide: config.fullscreenHide,
            },
            environment: {
              dpr: devicePixelRatio,
              width: innerWidth,
              height: innerHeight,
              documentWidth: document.documentElement.clientWidth,
              scrollbarWidth: innerWidth - document.documentElement.clientWidth,
              scrollHeight: document.documentElement.scrollHeight,
              scrollY,
              systemReducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
              visibility: document.visibilityState,
              focused: document.hasFocus(),
              font: getComputedStyle(document.body).fontFamily,
              fontsStatus: document.fonts.status,
              userAgent: navigator.userAgent,
              language: navigator.language,
            },
          };
        });
      } catch (problem) {
        windows[label] = { unavailable: failureCode(problem) };
      }
    }
    if (original) await browser.switchToWindow(original);
  } catch (problem) {
    windows.driver = { unavailable: failureCode(problem) };
  }
  const hints = diagnosticHints(host.report, windows.pet?.view);
  const report = {
    at: new Date().toISOString(),
    case: name,
    error,
    sourceCommit: process.env.CODE_MUSUME_SOURCE_REVISION ?? process.env.GITHUB_SHA ?? null,
    sourceDirty: process.env.CODE_MUSUME_SOURCE_DIRTY === "true",
    nodeVersion: process.version,
    windows,
    host,
    hints,
  };
  // The host request is independent of WebDriver: a failed WebView still leaves a core report.
  return report;
}
export async function saveUiFailure(index, name, error) {
  const report = await captureUiFailure(name, error);
  writeFileSync(
    join(process.env.CODE_MUSUME_UI_RUN_DIR, `failure-${index}.json`),
    JSON.stringify(report, null, 2),
  );
  console.error(
    `[diagnostic] ${name}: ${JSON.stringify({
      hints: report.hints,
      host: report.host.available,
      state: report.host.report?.projection?.state,
      pet: report.windows.pet?.view,
      environment: report.windows.pet?.environment,
    })}`,
  );
}
