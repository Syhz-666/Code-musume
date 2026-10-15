import { createHmac, randomBytes, randomUUID } from "node:crypto";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import http from "node:http";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const MAX_BODY = 2 * 1024 * 1024;
const knownTools = new Set([
  "Read",
  "Write",
  "Edit",
  "Bash",
  "Grep",
  "Glob",
  "Agent",
  "Task",
  "shell",
  "read_file",
  "exec_command",
  "functions.exec_command",
  "functions.apply_patch",
  "apply_patch",
]);
const safeKeys = new Set([
  "command",
  "cmd",
  "file_path",
  "path",
  "arguments",
  "description",
  "content",
  "output",
  "stdout",
  "stderr",
  "is_error",
  "isError",
  "exit_code",
  "exitCode",
  "error",
  "code",
  "status",
]);
export function sanitizer(salt = randomBytes(32)) {
  const token = (value) =>
    `sample-${createHmac("sha256", salt).update(String(value)).digest("hex").slice(0, 20)}`;
  function scrub(value, depth = 0) {
    if (depth > 24) throw new Error("sample-depth-limit");
    if (typeof value === "string") return token(value);
    if (Array.isArray(value)) {
      if (value.length > 256) throw new Error("sample-array-limit");
      return value.map((v) => scrub(v, depth + 1));
    }
    if (value && typeof value === "object")
      return Object.fromEntries(
        Object.entries(value).map(([k, v]) => [
          safeKeys.has(k) ? k : token(k),
          scrub(v, depth + 1),
        ]),
      );
    return value;
  }
  return (envelope) => {
    const raw = JSON.parse(envelope.payload),
      payload = {};
    for (const key of ["session_id", "prompt_id", "turn_id", "agent_id", "tool_use_id"]) {
      if (typeof raw[key] === "string") payload[key] = token(raw[key]);
    }
    if (typeof raw.tool_name === "string")
      payload.tool_name = knownTools.has(raw.tool_name) ? raw.tool_name : token(raw.tool_name);
    for (const key of ["tool_input", "tool_response", "background_tasks"])
      if (raw[key] !== undefined) payload[key] = scrub(raw[key]);
    if (typeof raw.notification_type === "string")
      payload.notification_type = [
        "permission_prompt",
        "agent_needs_input",
        "idle_prompt",
        "agent_completed",
      ].includes(raw.notification_type)
        ? raw.notification_type
        : "redacted-unsupported";
    payload.hook_event_name = envelope.event;
    return {
      v: 2,
      id: token(envelope.id),
      agent: envelope.agent,
      event: envelope.event,
      ts: envelope.ts,
      src: { cli: "real-redacted-capture", pid: 0 },
      payload: JSON.stringify(payload),
      parseError: envelope.parseError ?? null,
    };
  };
}
export function localRequest(runtime, method, path, body, timeout = 500) {
  return new Promise((done, reject) => {
    const request = http.request(
      {
        hostname: "127.0.0.1",
        port: runtime.port,
        method,
        path,
        agent: false,
        headers: {
          Authorization: `Bearer ${runtime.token}`,
          ...(body
            ? { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body) }
            : {}),
        },
      },
      (response) => {
        const chunks = [];
        let bytes = 0;
        response.on("data", (chunk) => {
          bytes += chunk.length;
          if (bytes > MAX_BODY) response.destroy(new Error("response-limit"));
          else chunks.push(chunk);
        });
        response.on("error", reject);
        response.on("end", () =>
          done({ status: response.statusCode, body: Buffer.concat(chunks).toString("utf8") }),
        );
      },
    );
    request.setTimeout(timeout, () => request.destroy(new Error("network-timeout")));
    request.on("error", reject);
    request.end(body);
  });
}
export async function startCapture({
  runtimePath,
  directory,
  runtimeDirectory = directory,
  seconds = 240,
  mode = "auto",
  versions = {},
}) {
  if (!Number.isFinite(seconds) || seconds < 1 || seconds > 3600)
    throw new Error("invalid-duration");
  const native = JSON.parse(readFileSync(runtimePath, "utf8").replace(/^\uFEFF/, ""));
  if (
    native.v !== 2 ||
    !Number.isInteger(native.port) ||
    native.port < 1 ||
    native.port > 65535 ||
    !/^[a-f0-9]{64}$/i.test(native.token)
  )
    throw new Error("invalid-runtime");
  const health = await localRequest(native, "GET", "/v2/health");
  if (health.status !== 200 || JSON.parse(health.body).projection.instanceId !== native.instanceId)
    throw new Error("native-readiness-failed");
  mkdirSync(directory, { recursive: true });
  mkdirSync(runtimeDirectory, { recursive: true });
  mode = mode === "auto" ? (runtimePath.endsWith("runtime.dev.json") ? "dev" : "release") : mode;
  const publishedPath = resolve(
    runtimeDirectory,
    mode === "dev" ? "runtime.dev.json" : "runtime.json",
  );
  if (existsSync(publishedPath) || existsSync(resolve(directory, "events.jsonl")))
    throw new Error("capture-directory-not-empty");
  const redact = sanitizer();
  const runtime = {
    ...native,
    token: randomBytes(32).toString("hex"),
    pid: process.pid,
    instanceId: randomUUID(),
    appVersion: "capture",
    startedAt: Date.now(),
  };
  let pending = 0,
    queuedBytes = 0,
    count = 0,
    failures = 0,
    chain = Promise.resolve(),
    stopping = false;
  const log = (record) =>
    appendFileSync(resolve(directory, "events.jsonl"), `${JSON.stringify(record)}\n`);
  const server = http.createServer(async (req, res) => {
    if (req.headers.origin !== undefined) {
      res.writeHead(403).end();
      return;
    }
    if (
      req.method !== "POST" ||
      req.url !== "/v2/event" ||
      req.headers.authorization !== `Bearer ${runtime.token}`
    ) {
      res.writeHead(401).end();
      return;
    }
    if (stopping) {
      res.writeHead(503).end();
      return;
    }
    const chunks = [];
    let bytes = 0;
    try {
      for await (const chunk of req) {
        bytes += chunk.length;
        if (bytes > MAX_BODY) {
          res.writeHead(413).end();
          return;
        }
        chunks.push(chunk);
      }
      if (pending >= 128 || queuedBytes + bytes > 8 * 1024 * 1024) {
        failures++;
        log({ stage: "rejected", outcome: "queue-full" });
        res.writeHead(429).end();
        return;
      }
      const raw = Buffer.concat(chunks).toString("utf8"),
        envelope = JSON.parse(raw);
      if (
        envelope.v !== 2 ||
        !["claude-code", "codex"].includes(envelope.agent) ||
        !/^[A-Za-z][A-Za-z0-9]{0,127}$/.test(envelope.event) ||
        typeof envelope.id !== "string" ||
        typeof envelope.payload !== "string" ||
        Buffer.byteLength(envelope.payload) > 256 * 1024
      )
        throw new Error("invalid-envelope");
      const sanitized = redact(envelope),
        captureId = sanitized.id;
      log({
        stage: "received",
        captureId,
        receivedAt: new Date().toISOString(),
        provenance: "real-redacted",
        sourceVersions: versions,
        envelope: sanitized,
      });
      count++;
      pending++;
      queuedBytes += bytes;
      res.writeHead(202).end();
      chain = chain.then(async () => {
        try {
          const before = JSON.parse(
            (await localRequest(native, "GET", "/v2/health")).body,
          ).projection;
          const forwarded = await localRequest(native, "POST", "/v2/event", raw);
          log({
            stage: "forwarded",
            captureId,
            status: forwarded.status,
            accepted: forwarded.status === 202,
          });
          if (forwarded.status !== 202) {
            failures++;
            return;
          }
          const after = JSON.parse((await localRequest(native, "GET", "/v2/health")).body);
          log({
            stage: "observed",
            captureId,
            beforeRevision: before.revision,
            changed: before.revision !== after.projection.revision,
            projection: after.projection,
            renderReceipt: after.renderReceipt ?? null,
          });
        } catch {
          failures++;
          log({ stage: "forward-failed", captureId, outcome: "native-request-failed" });
        } finally {
          pending--;
          queuedBytes -= bytes;
        }
      });
    } catch {
      failures++;
      log({ stage: "rejected", outcome: "invalid-or-unredactable-input" });
      if (!res.headersSent) res.writeHead(400).end();
    }
  });
  server.requestTimeout = 1000;
  server.headersTimeout = 1000;
  server.on("clientError", (_error, socket) => socket.destroy());
  await new Promise((ok, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", ok);
  });
  runtime.port = server.address().port;
  writeFileSync(publishedPath, JSON.stringify(runtime));
  writeFileSync(
    resolve(directory, "manifest.json"),
    JSON.stringify(
      {
        schemaVersion: 1,
        provenance: "real-redacted",
        startedAt: new Date().toISOString(),
        sourceVersions: versions,
        note: "Raw payloads are forwarded in memory only. Received, accepted and rendered evidence are separate.",
      },
      null,
      2,
    ),
  );
  let timer;
  let sampling = null;
  let lastDisplayKey = "";
  const displayTimer = setInterval(() => {
    if (sampling || stopping) return;
    sampling = localRequest(native, "GET", "/v2/health")
      .then((response) => {
        if (response.status !== 200) return;
        const snapshot = JSON.parse(response.body);
        if (snapshot.projection.instanceId !== native.instanceId) return;
        const key = `${snapshot.projection.revision}:${snapshot.renderReceipt?.revision ?? "none"}`;
        if (key !== lastDisplayKey) {
          lastDisplayKey = key;
          log({
            stage: "display-snapshot",
            observedAt: new Date().toISOString(),
            projection: snapshot.projection,
            renderReceipt: snapshot.renderReceipt ?? null,
          });
        }
      })
      .catch(() => {
        /* Event forwarding records failures separately. */
      })
      .finally(() => {
        sampling = null;
      });
  }, 250);
  const close = async () => {
    if (stopping) return;
    stopping = true;
    clearTimeout(timer);
    clearInterval(displayTimer);
    await new Promise((ok) => {
      server.close(ok);
      server.closeIdleConnections();
    });
    await chain;
    if (sampling) await sampling;
    let currentPublication;
    try {
      currentPublication = JSON.parse(readFileSync(publishedPath, "utf8"));
    } catch {
      /* Preserve missing or externally edited files. */
    }
    if (currentPublication?.instanceId === runtime.instanceId) unlinkSync(publishedPath);
    writeFileSync(
      resolve(directory, "result.json"),
      JSON.stringify(
        {
          received: count,
          failures,
          nativeInstanceId: native.instanceId,
          sourceVersions: versions,
          finishedAt: new Date().toISOString(),
        },
        null,
        2,
      ),
    );
  };
  timer = setTimeout(() => {
    void close();
  }, seconds * 1000);
  return { runtime, publishedPath, close };
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = Object.fromEntries(
    Array.from({ length: Math.floor((process.argv.length - 2) / 2) }, (_, i) => [
      process.argv[2 + i * 2].slice(2),
      process.argv[3 + i * 2],
    ]),
  );
  try {
    const capture = await startCapture({
      runtimePath: args.runtime,
      directory: args.dir,
      seconds: Number(args.seconds ?? 240),
      mode: args.mode ?? "auto",
      versions: JSON.parse(args.versions ?? "{}"),
    });
    console.log(`Capture ready: ${capture.publishedPath}`);
    for (const signal of ["SIGINT", "SIGTERM"])
      process.once(signal, () => {
        void capture.close();
      });
  } catch (error) {
    console.error(`Capture failed: ${error.message}`);
    process.exitCode = 1;
  }
}
