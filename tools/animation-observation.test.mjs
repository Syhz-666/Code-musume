import assert from "node:assert/strict";
import test from "node:test";
import { assertEntryTiming } from "../tests/desktop/animation-observation.mjs";

function trace(samples) {
  return {
    samples: samples.map(([frame, atMs, observedAtMs]) => ({ frame, atMs, observedAtMs, fps: 48 })),
    firstReady: { frame: 9, atMs: 1200 },
  };
}
test("entry evidence survives late observation and late driver reads", () => {
  assertEntryTiming(
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
      assertEntryTiming(
        trace([
          [1, 1000, 1000],
          [13, 1250, 1250],
        ]),
      ),
    /frame zero/,
  );
  assert.throws(
    () =>
      assertEntryTiming(
        trace([
          [0, 1000, 1000],
          [40, 1250, 1250],
        ]),
      ),
    /jumped ahead/,
  );
});

function handoffTrace(firstAtMs, firstFrame, nextAtMs = 1800, nextFrame = 7) {
  const next = { frame: nextFrame, fps: 24, atMs: nextAtMs, ready: true };
  return {
    segment: "notification-enter",
    exitSegment: "waiting-permission-exit",
    exitSamples: [{ frame: 0, frames: 24, fps: 48, atMs: 1000 }],
    samples: [{ frame: firstFrame, fps: 24, atMs: firstAtMs, ready: false }, next],
    firstReady: next,
  };
}

test("entry after a 500ms exit uses the elapsed time beyond the handoff", () => {
  assertEntryTiming(handoffTrace(1541, 0));
  assertEntryTiming(handoffTrace(1545, 1));
});

test("handoff evidence rejects missing exits, early entries, incorrect frames and stalls", () => {
  assert.throws(
    () => assertEntryTiming({ ...handoffTrace(1545, 1), exitSamples: [] }),
    /Missing exit samples/,
  );
  assert.throws(() => assertEntryTiming(handoffTrace(1499, 0)), /began before/);
  assert.throws(() => assertEntryTiming(handoffTrace(1545, 2)), /match the exit timeline/);
  assert.throws(() => assertEntryTiming(handoffTrace(1545, 1, 1565, 1)), /must advance/);
});
