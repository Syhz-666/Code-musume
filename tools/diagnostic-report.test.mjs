import assert from "node:assert/strict";
import test from "node:test";
import { diagnosticHints, failureCode, readPipelineDiagnostics } from "./diagnostic-report.mjs";

test("explains invalid input and motion preference instead of a generic render timeout", () => {
  const hints = diagnosticHints(
    {
      pipeline: {
        entries: [
          {
            stage: "http-receive",
            eventId: "abc",
            input: { jsonValid: false, errorKind: "utf8-bom", errorLine: 1, errorColumn: 1 },
          },
          { stage: "core", eventId: "abc", outcome: "invalid-payload" },
        ],
      },
    },
    {
      animationRunning: "false",
      systemReducedMotion: "true",
      revision: "2",
      renderedRevision: "1",
    },
  );
  assert(hints.some((h) => h.includes("utf8-bom")));
  assert(hints.includes("core:invalid-payload;event=abc"));
  assert(hints.includes("animation:system-reduced-motion"));
  assert(hints.some((h) => h.includes("revision-not-submitted")));
});
test("missing runtime and raw exceptions do not expose a local path or secret", async () => {
  assert.equal(failureCode(new Error("token=secret C:/Users/private")), "unavailable");
  const report = await readPipelineDiagnostics("does-not-exist-for-diagnostic-test.json");
  assert.deepEqual(report, { available: false, reason: "ENOENT" });
});
