import assert from "node:assert/strict";

// Observe inside the WebView before issuing the command. Remote reads may arrive
// hundreds of milliseconds later, so the current frame cannot prove where it began.
export async function observePreviewEntry(state, segment, { exitSegment = null } = {}) {
  return browser.execute(
    (state, segment, exitSegment) =>
      new Promise((resolve, reject) => {
        const samples = [];
        const exitSamples = [];
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
                `Animation entry not observed: ${segment}; trace=${JSON.stringify({ exitSegment, exitSamples, samples })}`,
              ),
            ),
          1500,
        );
        const onFrame = (event) => {
          const data = event.detail;
          if (data.state !== state) return;
          const exiting = data.segment === exitSegment && data.phase === "exit";
          const entering = data.segment === segment && data.phase === "enter";
          if (!exiting && !entering) return;
          const sample = {
            frame: data.frame,
            frames: data.frames,
            fps: data.fps,
            atMs: data.atMs,
            observedAtMs: performance.now(),
            ready: data.ready,
          };
          if (!Number.isFinite(sample.atMs) || !Number.isFinite(sample.fps)) {
            fail(new Error("Missing e2e frame timing; rebuild the test application"));
            return;
          }
          const observed = exiting ? exitSamples : samples;
          if (observed.at(-1)?.atMs === sample.atMs) return;
          observed.push(sample);
          if (entering && sample.ready && !firstReady) firstReady = sample;
          if (entering && firstReady && sample.atMs - samples[0].atMs >= 250) {
            cleanup();
            resolve({ state, segment, exitSegment, exitSamples, samples, firstReady });
          } else if (exitSamples.length + samples.length >= 80) {
            fail(new Error("Animation observation exceeded its bounded sample limit"));
          }
        };
        window.addEventListener("code-musume:e2e-frame", onFrame);
        window.__TAURI_INTERNALS__.invoke("set_animation_preview_state", { state }).catch(fail);
      }),
    state,
    segment,
    exitSegment,
  );
}

export function assertEntryTiming(trace) {
  const start = trace.samples[0];
  assert(start, "The entry must have a recorded first frame");
  let entryAtMs = start.atMs;
  if (trace.exitSegment) {
    const exit = trace.exitSamples?.[0];
    assert(exit, `Missing exit samples for ${trace.exitSegment}`);
    assert.equal(exit.frame, 0, `${trace.exitSegment} must start at frame zero`);
    assert(Number.isInteger(exit.frames) && exit.frames > 0, "Invalid exit frame count");
    assert(Number.isFinite(exit.fps) && exit.fps > 0, "Invalid exit frame rate");
    entryAtMs = exit.atMs + (exit.frames * 1000) / exit.fps;
    assert(start.atMs >= entryAtMs, `${trace.segment} began before ${trace.exitSegment} ended`);
  } else {
    assert.equal(start.frame, 0, "Direct entry must render frame zero first");
  }
  assert(trace.firstReady, "The entry must finish blending into the requested pose");
  let previousAtMs = start.atMs;
  let previousFrame = start.frame;
  for (const sample of trace.samples) {
    assert.equal(sample.fps, start.fps);
    assert(sample.atMs >= previousAtMs, "Frame timestamps must be monotonic");
    assert(sample.frame >= previousFrame, "Entry frame numbers must be monotonic");
    previousAtMs = sample.atMs;
    previousFrame = sample.frame;
    const elapsedMs = sample.atMs - entryAtMs;
    const expectedFrame = Math.floor((elapsedMs * start.fps) / 1000);
    if (trace.exitSegment) {
      assert.equal(
        sample.frame,
        expectedFrame,
        `${trace.segment} frame must match the exit timeline at ${elapsedMs}ms after the handoff`,
      );
    } else {
      const maximumFrame = expectedFrame + 1;
      assert(
        sample.frame <= maximumFrame,
        `Entry jumped ahead: frame=${sample.frame}, elapsedMs=${elapsedMs}, maximumFrame=${maximumFrame}`,
      );
    }
  }
  assert(previousFrame > start.frame, "The entry must advance after starting");
}
