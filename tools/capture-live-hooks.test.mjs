import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import http from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { localRequest, sanitizer, startCapture } from "./capture-live-hooks.mjs";

test("stale runtime fails readiness without publishing a capture", async () => {
  const root = mkdtempSync(join(tmpdir(), "musume-stale-"));
  const server = http.createServer((_req, res) =>
    res.end(JSON.stringify({ projection: { instanceId: "other" } })),
  );
  await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
  writeFileSync(
    join(root, "native.json"),
    JSON.stringify({
      v: 2,
      port: server.address().port,
      token: "a".repeat(64),
      instanceId: "expected",
    }),
  );
  try {
    await assert.rejects(
      startCapture({ runtimePath: join(root, "native.json"), directory: join(root, "capture") }),
      /native-readiness-failed/,
    );
    assert(!existsSync(join(root, "capture/runtime.json")));
  } finally {
    await new Promise((ok) => server.close(ok));
  }
});

test("accepted forwarding is distinct from observation and external runtime edits survive cleanup", async () => {
  const root = mkdtempSync(join(tmpdir(), "musume-accepted-"));
  let revision = 0;
  let state = "working";
  let originalReachedNative = false;
  const server = http.createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => {
      body += chunk;
    });
    req.on("end", () => {
      if (req.method === "POST") {
        originalReachedNative = body.includes("SECRET input");
        revision++;
        setTimeout(() => {
          revision = 2;
          state = "idle";
        }, 60);
        res.writeHead(202);
        res.end('{"accepted":true}');
      } else
        res.end(
          JSON.stringify({
            projection: { instanceId: "native", revision, state },
            renderReceipt:
              revision === 2 ? { instanceId: "native", revision: 2, latencyMs: 12 } : null,
          }),
        );
    });
  });
  await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
  const runtimePath = join(root, "runtime.dev.json");
  writeFileSync(
    runtimePath,
    JSON.stringify({
      v: 2,
      port: server.address().port,
      token: "b".repeat(64),
      instanceId: "native",
    }),
  );
  let capture;
  try {
    capture = await startCapture({
      runtimePath,
      directory: join(root, "run"),
      runtimeDirectory: join(root, "published"),
      seconds: 30,
    });
    assert(capture.publishedPath.endsWith("runtime.dev.json"));
    const response = await localRequest(
      capture.runtime,
      "POST",
      "/v2/event",
      JSON.stringify({
        v: 2,
        id: "one",
        agent: "codex",
        event: "PreToolUse",
        ts: 0,
        payload: JSON.stringify({
          session_id: "s",
          turn_id: "t",
          tool_input: { command: "SECRET input" },
        }),
      }),
    );
    assert.equal(response.status, 202);
    writeFileSync(capture.publishedPath, "external edit that is not JSON");
    const until = Date.now() + 3000;
    while (
      Date.now() < until &&
      !readFileSync(join(root, "run/events.jsonl"), "utf8").includes('"stage":"display-snapshot"')
    )
      await new Promise((ok) => setTimeout(ok, 20));
    await capture.close();
    assert.equal(readFileSync(capture.publishedPath, "utf8"), "external edit that is not JSON");
    assert(originalReachedNative);
    const text = readFileSync(join(root, "run/events.jsonl"), "utf8");
    assert(!text.includes("SECRET"));
    const allRecords = text.trim().split("\n").map(JSON.parse);
    const sampled = allRecords.find((e) => e.stage === "display-snapshot");
    assert.equal(sampled?.projection.state, "idle");
    assert.equal(sampled?.renderReceipt.revision, 2);
    const records = allRecords.filter((e) => e.captureId);
    assert.deepEqual(
      records.map((e) => e.stage),
      ["received", "forwarded", "observed"],
    );
    assert.equal(records[1].accepted, true);
    assert.equal(records[2].changed, true);
    assert.equal(records[2].renderReceipt, null);
  } finally {
    await capture?.close();
    await new Promise((ok) => server.close(ok));
  }
});

test("redaction preserves correlation and failure flags without saving source content", () => {
  const redact = sanitizer(Buffer.alloc(32, 1));
  const event = {
    v: 2,
    id: "id",
    agent: "codex",
    event: "PostToolUse",
    ts: 0,
    payload: JSON.stringify({
      session_id: "private-session",
      turn_id: "turn",
      tool_name: "shell",
      tool_input: { command: "SECRET command", privateField: "SECRET prompt" },
      tool_response: { exit_code: 2, is_error: true, output: "SECRET code" },
      prompt: "SECRET prompt",
      cwd: "SECRET path",
    }),
  };
  const sample = redact(event);
  assert(!JSON.stringify(sample).includes("SECRET"));
  assert(!JSON.stringify(sample).includes("privateField"));
  const p = JSON.parse(sample.payload);
  assert.equal(p.tool_response.exit_code, 2);
  assert.equal(p.tool_response.is_error, true);
  assert.deepEqual(sample, redact(event));
  assert.equal(p.cwd, undefined);
});
test("forwarding rejection is recorded and capture runtime is removed", async () => {
  const root = mkdtempSync(join(tmpdir(), "musume-capture-"));
  const native = { v: 2, port: 0, token: "a".repeat(64), instanceId: "native" };
  const server = http.createServer((req, res) => {
    req.resume();
    res.writeHead(req.method === "GET" ? 200 : 429, { "content-type": "application/json" });
    res.end(JSON.stringify({ projection: { instanceId: "native", revision: 1 } }));
  });
  await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
  native.port = server.address().port;
  const runtime = join(root, "native.json");
  writeFileSync(runtime, JSON.stringify(native));
  let capture;
  try {
    capture = await startCapture({
      runtimePath: runtime,
      directory: join(root, "capture"),
      seconds: 30,
    });
    const response = await localRequest(
      capture.runtime,
      "POST",
      "/v2/event",
      JSON.stringify({
        v: 2,
        id: "id",
        agent: "codex",
        event: "Stop",
        ts: 0,
        payload: '{"session_id":"secret"}',
      }),
    );
    assert.equal(response.status, 202);
    await capture.close();
    assert(!existsSync(capture.publishedPath));
    const lines = readFileSync(join(root, "capture/events.jsonl"), "utf8")
      .trim()
      .split("\n")
      .map(JSON.parse)
      .filter((e) => e.captureId);
    assert.equal(lines[0].stage, "received");
    assert.equal(lines[1].accepted, false);
    assert.equal(lines[1].status, 429);
    assert.equal(JSON.parse(readFileSync(join(root, "capture/result.json"))).failures, 1);
  } finally {
    await capture?.close();
    await new Promise((ok) => server.close(ok));
  }
});
