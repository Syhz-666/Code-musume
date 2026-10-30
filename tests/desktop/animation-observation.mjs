import assert from "node:assert/strict";

// Observe inside the WebView before issuing the command. Remote reads may arrive
// hundreds of milliseconds later, so the current frame cannot prove where it began.
export async function observePreviewEntry(state, segment) {
  return browser.execute(
    (state, segment) =>
      new Promise((resolve, reject) => {
        const samples = [];
        let firstReady = null;
        const cleanup = () => {
          window.removeEventListener("code-musume:e2e-frame", onFrame);
          clearTimeout(timer);
        };
        const fail = (error) => {
          cleanup();
          reject(error);
        };
        const timer = setTimeout(
          () =>
            fail(
              new Error(
                `Animation entry not observed: ${segment}; samples=${JSON.stringify(samples)}`,
              ),
            ),
          1500,
        );
        const onFrame = (event) => {
          const data = event.detail;
          if (data.state !== state || data.segment !== segment || data.phase !== "enter") return;
          const sample = {
            frame: data.frame,
            fps: data.fps,
            atMs: data.atMs,
            observedAtMs: performance.now(),
            ready: data.ready,
          };
          if (!Number.isFinite(sample.atMs) || !Number.isFinite(sample.fps)) {
            fail(new Error("Missing e2e frame timing; rebuild the test application"));
            return;
          }
          if (samples.at(-1)?.atMs === sample.atMs) return;
          samples.push(sample);
          if (sample.ready && !firstReady) firstReady = sample;
          if (firstReady && sample.atMs - samples[0].atMs >= 250) {
            cleanup();
            resolve({ state, segment, samples, firstReady });
          } else if (samples.length >= 80) {
            fail(new Error("Animation observation exceeded its bounded sample limit"));
          }
        };
        window.addEventListener("code-musume:e2e-frame", onFrame);
        window.__TAURI_INTERNALS__.invoke("set_animation_preview_state", { state }).catch(fail);
      }),
    state,
    segment,
  );
}

export function assertEntryStartsAtBeginning(trace) {
  const start = trace.samples[0];
  assert(start, "The entry must have a recorded first frame");
  assert.equal(start.frame, 0, "Waking from low FPS must render entry frame zero first");
  assert(trace.firstReady, "The entry must finish blending into the requested pose");
  let previousAtMs = start.atMs;
  for (const sample of trace.samples) {
    assert.equal(sample.fps, start.fps);
    assert(sample.atMs >= previousAtMs, "Frame timestamps must be monotonic");
    previousAtMs = sample.atMs;
    const elapsedMs = sample.atMs - start.atMs;
    // One frame covers rounding between clock samples; driver latency is excluded.
    const maximumFrame = Math.floor((elapsedMs * start.fps) / 1000) + 1;
    assert(
      sample.frame <= maximumFrame,
      `Entry jumped ahead: frame=${sample.frame}, elapsedMs=${elapsedMs}, maximumFrame=${maximumFrame}`,
    );
  }
  assert(
    trace.samples.some((sample) => sample.frame > 0),
    "The entry must advance after starting",
  );
}
