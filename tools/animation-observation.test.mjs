import assert from "node:assert/strict";
import test from "node:test";
import { assertEntryStartsAtBeginning } from "../tests/desktop/animation-observation.mjs";

function trace(samples) {
  return {
    samples: samples.map(([frame, atMs, observedAtMs]) => ({ frame, atMs, observedAtMs, fps: 48 })),
    firstReady: { frame: 9, atMs: 1200 },
  };
}
test("entry evidence survives late observation and late driver reads", () => {
  assertEntryStartsAtBeginning(
    trace([
      [0, 1000, 1180],
      [9, 1200, 1205],
      [12, 1250, 1500],
    ]),
  );
});
test("entry evidence rejects skipped starts and a sleeping interval charged to new frames", () => {
  assert.throws(
    () =>
      assertEntryStartsAtBeginning(
        trace([
          [25, 1000, 1000],
          [37, 1250, 1250],
        ]),
      ),
    /frame zero/,
  );
  assert.throws(
    () =>
      assertEntryStartsAtBeginning(
        trace([
          [0, 1000, 1000],
          [40, 1250, 1250],
        ]),
      ),
    /jumped ahead/,
  );
});
