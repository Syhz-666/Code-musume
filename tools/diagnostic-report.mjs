import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { localRequest } from "./capture-live-hooks.mjs";

const safeCodes = new Set(["ENOENT", "EACCES", "ECONNREFUSED", "ECONNRESET", "ETIMEDOUT"]);
export function failureCode(error) {
  return safeCodes.has(error?.code) ? error.code : "unavailable";
}
export async function readPipelineDiagnostics(runtimePath) {
  try {
    const runtime = JSON.parse(readFileSync(runtimePath, "utf8"));
    const response = await localRequest(runtime, "GET", "/v2/diagnostics", undefined, 2000);
    if (response.status !== 200)
      return { available: false, reason: "http-rejected", status: response.status };
    return { available: true, report: JSON.parse(response.body) };
  } catch (error) {
    return { available: false, reason: failureCode(error) };
  }
}
export function diagnosticHints(report, view = {}) {
  const hints = [];
  for (const entry of report?.pipeline?.entries ?? []) {
    if (entry.input && !entry.input.jsonValid)
      hints.push(
        `input-invalid:${entry.input.errorKind};event=${entry.eventId ?? "unknown"};line=${entry.input.errorLine};column=${entry.input.errorColumn}`,
      );
    if (entry.stage === "core" && entry.outcome !== "applied")
      hints.push(`core:${entry.outcome};event=${entry.eventId}`);
  }
  if (view.animationRunning === "false") {
    if (view.systemReducedMotion === "true") hints.push("animation:system-reduced-motion");
    if (view.configuredReducedMotion === "true") hints.push("animation:application-reduced-motion");
  }
  if (view.animationMode === "manual") hints.push("display:manual-preview-active");
  if (view.revision && view.revision !== view.renderedRevision)
    hints.push(
      `display:revision-not-submitted;expected=${view.revision};submitted=${view.renderedRevision ?? "none"}`,
    );
  return [...new Set(hints)].slice(-16);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const option = (key) => args[args.indexOf(key) + 1];
  const mode = args.includes("--dev") ? "runtime.dev.json" : "runtime.json";
  const runtimePath = args.includes("--runtime-file")
    ? resolve(option("--runtime-file"))
    : join(
        process.env.CODE_MUSUME_DATA_DIR ?? join(process.env.LOCALAPPDATA ?? "", "code-musume"),
        mode,
      );
  const target = resolve(
    args.includes("--out") ? option("--out") : ".test-artifacts/diagnostics/latest.json",
  );
  if (resolve(runtimePath) === target)
    throw new Error("Diagnostic output must differ from runtime file");
  const diagnostic = await readPipelineDiagnostics(runtimePath);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(
    target,
    JSON.stringify({ capturedAt: new Date().toISOString(), ...diagnostic }, null, 2),
  );
  console.log(
    JSON.stringify({
      available: diagnostic.available,
      reason: diagnostic.reason,
      state: diagnostic.report?.projection?.state,
      hints: diagnosticHints(diagnostic.report),
    }),
  );
  if (!diagnostic.available) process.exitCode = 1;
}
