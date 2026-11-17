import { readFileSync } from "node:fs";
import { createServer } from "node:net";
import { codexTestPipe } from "./environment.mjs";

const fixture = readFileSync(
  new URL("../../fixtures/codex-sync/progress-v13.jsonl", import.meta.url),
  "utf8",
)
  .trim()
  .split(/\r?\n/)
  .map((line) => JSON.parse(line));

export async function startCodexPeer() {
  const peers = new Set();
  const followed = new Map();
  const server = createServer((socket) => {
    peers.add(socket);
    socket.on("error", () => socket.destroy());
    socket.on("close", () => peers.delete(socket));
    let pending = Buffer.alloc(0);
    socket.on("data", (chunk) => {
      pending = Buffer.concat([pending, chunk]);
      while (pending.length >= 4 && pending.length >= 4 + pending.readUInt32LE(0)) {
        const end = 4 + pending.readUInt32LE(0);
        const frame = JSON.parse(pending.subarray(4, end).toString());
        pending = pending.subarray(end);
        if (frame.method === "initialize") {
          send(socket, {
            type: "response",
            requestId: frame.requestId,
            resultType: "success",
            result: { clientId: "e2e-observer" },
          });
        }
        if (frame.method === "thread-stream-following-changed" && frame.params.hostId === "local") {
          if (frame.params.following) followed.set(frame.params.conversationId, socket);
          else followed.delete(frame.params.conversationId);
        }
      }
    });
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(codexTestPipe, resolve);
  });
  function send(socket, frame) {
    const payload = Buffer.from(JSON.stringify(frame));
    const prefix = Buffer.alloc(4);
    prefix.writeUInt32LE(payload.length);
    socket.write(Buffer.concat([prefix, payload]));
  }
  return {
    following: (session) => followed.has(session) && !followed.get(session).destroyed,
    beginFinalOnly(session) {
      send(followed.get(session), {
        ...fixture[0],
        params: {
          ...fixture[0].params,
          conversationId: session,
          change: {
            type: "snapshot",
            revision: 1,
            conversationState: {
              requests: [],
              turns: [{ turnId: "final-turn", status: "inProgress", items: [] }],
            },
          },
        },
      });
    },
    finishFinalOnly(session) {
      send(followed.get(session), {
        ...fixture[0],
        params: {
          ...fixture[0].params,
          conversationId: session,
          change: {
            type: "patches",
            baseRevision: 1,
            revision: 2,
            patches: [
              { op: "replace", path: ["turns", 0, "status"], value: "completed" },
              {
                op: "add",
                path: ["turns", 0, "items", 0],
                value: {
                  type: "agentMessage",
                  id: "final-message",
                  phase: "final_answer",
                  completedAtMs: 2,
                  text: "登录测试通过，保存设置还没验证。",
                },
              },
            ],
          },
        },
      });
    },
    replay(session) {
      for (const frame of fixture) {
        send(followed.get(session), {
          ...frame,
          params: { ...frame.params, conversationId: session },
        });
      }
    },
    requestApproval(session) {
      const frame = fixture.at(-1);
      const baseRevision = frame.params.change.revision;
      send(followed.get(session), {
        ...frame,
        params: {
          ...frame.params,
          conversationId: session,
          change: {
            type: "patches",
            baseRevision,
            revision: baseRevision + 1,
            patches: [
              {
                op: "add",
                path: ["requests", 0],
                value: { method: "item/commandExecution/requestApproval" },
              },
            ],
          },
        },
      });
    },
    async close() {
      for (const peer of peers) peer.destroy();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
