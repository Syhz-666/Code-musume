import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { startCapture } from "./capture-live-hooks.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const command = process.argv[2];
const supplied = process.argv.indexOf("--dir");
const directory =
  supplied >= 0
    ? resolve(process.argv[supplied + 1])
    : join(root, ".test-artifacts", `live-${Date.now()}`);
const project = join(directory, "project");
const ccEvents = [
  "SessionStart",
  "SessionEnd",
  "UserPromptSubmit",
  "PreToolUse",
  "PostToolUse",
  "PostToolUseFailure",
  "PermissionRequest",
  "PermissionDenied",
  "Notification",
  "Stop",
  "StopFailure",
  "SubagentStart",
  "SubagentStop",
  "PreCompact",
  "PostCompact",
];
const cxEvents = [
  "SessionStart",
  "SessionEnd",
  "UserPromptSubmit",
  "PreToolUse",
  "PostToolUse",
  "PermissionRequest",
  "Stop",
  "Interrupt",
  "SubagentStart",
  "SubagentStop",
  "PreCompact",
  "PostCompact",
];
const literal = (s) => `'${s.replaceAll("'", "''")}'`;
const capturePath = join(directory, "capture/runtime.dev.json");
const cli = join(root, "target/debug/code-musume-hook.exe");
const claudeEntry = join(
  process.env.APPDATA ?? "",
  "npm/node_modules/@anthropic-ai/claude-code/bin/claude.exe",
);
function hooks(agent, events) {
  return Object.fromEntries(
    events.map((event) => {
      const args = [
        "--agent",
        agent,
        "--event",
        event,
        "--runtime",
        "dev",
        "--runtime-file",
        capturePath,
      ];
      if (agent === "codex" && event === "SessionEnd") args.push("--budget-ms", "100");
      let handler;
      if (agent === "claude-code")
        handler = { type: "command", command: cli, args, async: true, timeout: 5 };
      else {
        const script = `try { & ${[cli, ...args].map(literal).join(" ")} 1>$null 2>$null } catch {}; exit 0 # code-musume isolated capture`;
        const command = `powershell.exe -NoProfile -NonInteractive -EncodedCommand ${Buffer.from(script, "utf16le").toString("base64")}`;
        handler = {
          type: "command",
          command,
          commandWindows: command,
          async: event !== "SessionEnd",
          timeout: event === "SessionEnd" || event === "Interrupt" ? 1 : 5,
        };
      }
      return [event, [{ hooks: [handler] }]];
    }),
  );
}
function versions() {
  const cc = spawnSync(claudeEntry, ["--version"], {
    encoding: "utf8",
    windowsHide: true,
  });
  const bin = join(process.env.LOCALAPPDATA ?? "", "OpenAI/Codex/bin");
  const directories = existsSync(bin)
    ? readdirSync(bin)
        .map((name) => join(bin, name))
        .filter((p) => existsSync(join(p, "codex.exe")))
        .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)
    : [];
  const cx = directories.length
    ? spawnSync(join(directories[0], "codex.exe"), ["--version"], {
        encoding: "utf8",
        windowsHide: true,
      })
    : null;
  return {
    "claude-code": cc.status === 0 ? cc.stdout.trim() : "unavailable",
    "codex-desktop-runtime": cx?.status === 0 ? cx.stdout.trim() : "unavailable",
  };
}
try {
  if (command === "prepare") {
    if (!existsSync(cli)) throw new Error("Build code-musume-hook first");
    mkdirSync(join(project, ".claude"), { recursive: true });
    mkdirSync(join(project, ".codex"), { recursive: true });
    if (!existsSync(join(project, ".git"))) {
      const initialized = spawnSync(
        "git",
        ["init", "--initial-branch=codex/live-acceptance", project],
        { encoding: "utf8", windowsHide: true },
      );
      if (initialized.status !== 0)
        throw new Error("Could not initialize the isolated test repository");
    }
    writeFileSync(
      join(project, "fixture.txt"),
      "Code-musume integration fixture: MUSUME-20261015\nThis file contains no user code or credentials.\n",
    );
    writeFileSync(
      join(project, ".claude/settings.json"),
      JSON.stringify({ hooks: hooks("claude-code", ccEvents) }, null, 2),
    );
    writeFileSync(
      join(project, ".codex/hooks.json"),
      JSON.stringify({ hooks: hooks("codex", cxEvents) }, null, 2),
    );
    const hashes = Object.fromEntries(
      [".claude/settings.json", ".codex/hooks.json"].map((p) => [
        p,
        createHash("sha256")
          .update(readFileSync(join(project, p)))
          .digest("hex"),
      ]),
    );
    writeFileSync(
      join(directory, "session.json"),
      JSON.stringify(
        {
          schemaVersion: 1,
          project,
          versions: versions(),
          hookDefinitions: hashes,
          status: "prepared-not-validated",
        },
        null,
        2,
      ),
    );
    writeFileSync(
      join(project, "README.md"),
      "# 隔离联动验收\n\n在 Codex 桌面打开此项目，确认任务由本机编排，按应用支持的入口审阅并信任本项目 hooks。项目本身的信任和 hook 定义信任是两个检查。\n\n正常流程：读取 fixture.txt，报告其中的编号，不修改文件。\n\n中断流程：开始一项较长的只读检查，手动停止。权限和子 agent 只使用当前版本能够实际触发的操作。\n\n不要编辑信任记录，不使用绕过信任的参数。切换聊天不等同 SessionEnd。\n",
    );
    console.log(
      `Prepared isolated project: ${project}\nStart observer: node tools/live-session.mjs serve --dir "${directory}"`,
    );
  } else if (command === "serve") {
    const metadata = JSON.parse(readFileSync(join(directory, "session.json"), "utf8"));
    metadata.versions = versions();
    metadata.activeCaptureDirectory = join(directory, "capture/runs", String(Date.now()));
    writeFileSync(join(directory, "session.json"), JSON.stringify(metadata, null, 2));
    const env = {
      ...process.env,
      CODE_MUSUME_DATA_DIR: join(directory, "runtime"),
      CODE_MUSUME_CONFIG_DIR: join(directory, "config"),
      WEBVIEW2_USER_DATA_FOLDER: join(directory, "webview"),
    };
    mkdirSync(env.CODE_MUSUME_CONFIG_DIR, { recursive: true });
    if (!existsSync(join(env.CODE_MUSUME_CONFIG_DIR, "config.json")))
      writeFileSync(
        join(env.CODE_MUSUME_CONFIG_DIR, "config.json"),
        JSON.stringify({ fullscreenHide: false }),
      );
    const binaryArgument = process.argv.indexOf("--binary");
    const binary =
      binaryArgument >= 0
        ? resolve(process.argv[binaryArgument + 1])
        : join(root, "target/debug/pet.exe");
    const stagedDirectory = join(directory, "application", String(Date.now()));
    mkdirSync(stagedDirectory, { recursive: true });
    const stagedBinary = join(stagedDirectory, "pet.exe");
    copyFileSync(binary, stagedBinary);
    copyFileSync(cli, join(stagedDirectory, "code-musume-hook.exe"));
    metadata.applicationSha256 = createHash("sha256")
      .update(readFileSync(stagedBinary))
      .digest("hex");
    writeFileSync(join(directory, "session.json"), JSON.stringify(metadata, null, 2));
    const app = spawn(stagedBinary, [], {
      env,
      windowsHide: true,
      stdio: "ignore",
    });
    const runtimePath = join(directory, "runtime/runtime.dev.json");
    const until = Date.now() + 15000;
    while (true) {
      let published;
      try {
        published = JSON.parse(readFileSync(runtimePath, "utf8"));
      } catch {
        /* Startup has not published yet. */
      }
      if (published?.pid === app.pid) break;
      if (app.exitCode !== null || Date.now() > until)
        throw new Error("live application did not become ready");
      await new Promise((ok) => setTimeout(ok, 100));
    }
    let capture;
    try {
      capture = await startCapture({
        runtimePath,
        directory: metadata.activeCaptureDirectory,
        runtimeDirectory: join(directory, "capture"),
        seconds: 1800,
        versions: metadata.versions,
      });
      console.log(
        `Observer ready. Open Codex local task in: ${project}\nHook definitions require review and trust in Codex. No trust records were modified.`,
      );
      const stop = async () => {
        await capture.close();
        app.kill();
      };
      for (const signal of ["SIGINT", "SIGTERM"])
        process.once(signal, () => {
          void stop();
        });
      const guard = setInterval(() => {
        if (!existsSync(capture.publishedPath)) {
          clearInterval(guard);
          app.kill();
        }
      }, 1000);
      app.once("exit", () => {
        clearInterval(guard);
        void capture.close();
      });
    } catch (error) {
      app.kill();
      throw error;
    }
  } else if (command === "claude") {
    if (!existsSync(capturePath)) throw new Error("Start the observer first");
    const child = spawn(
      claudeEntry,
      [
        "-p",
        "使用 Read 工具读取当前目录的 fixture.txt，然后只报告其中的编号。不要修改任何文件。",
        "--allowedTools",
        "Read",
        "--disallowedTools",
        "Bash,Write,Edit",
        "--no-session-persistence",
        "--output-format",
        "json",
        "--settings",
        join(project, ".claude/settings.json"),
      ],
      { cwd: project, windowsHide: true, stdio: ["ignore", "ignore", "pipe"] },
    );
    let errorBytes = 0;
    child.stderr.on("data", (chunk) => {
      errorBytes += chunk.length;
    });
    const timer = setTimeout(() => child.kill(), 120000);
    child.once("exit", (code) => {
      clearTimeout(timer);
      writeFileSync(
        join(directory, "claude-run.json"),
        JSON.stringify(
          {
            exitCode: code,
            stderrBytes: errorBytes,
            fixtureOnly: true,
            at: new Date().toISOString(),
          },
          null,
          2,
        ),
      );
      if (code !== 0) process.exitCode = 1;
    });
  } else if (command === "report") {
    const metadata = JSON.parse(readFileSync(join(directory, "session.json"), "utf8"));
    const events = readFileSync(
      join(metadata.activeCaptureDirectory ?? join(directory, "capture"), "events.jsonl"),
      "utf8",
    )
      .trim()
      .split("\n")
      .filter(Boolean)
      .map(JSON.parse);
    const accepted = new Set(
      events.filter((e) => e.stage === "forwarded" && e.accepted).map((e) => e.captureId),
    );
    const result = {};
    for (const agent of ["claude-code", "codex"]) {
      const kinds = [
        ...new Set(
          events
            .filter(
              (e) =>
                e.stage === "received" && e.envelope.agent === agent && accepted.has(e.captureId),
            )
            .map((e) => e.envelope.event),
        ),
      ];
      result[agent] = {
        observedEvents: kinds,
        normalEventsComplete: ["UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop"].every((k) =>
          kinds.includes(k),
        ),
      };
    }
    result.jointWork = "requires-ordered-real-session-and-render-evidence";
    result.manual = "pending-user-checklist";
    writeFileSync(join(directory, "live-report.json"), JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result, null, 2));
  } else throw new Error("Usage: live-session.mjs prepare|serve|claude|report [--dir PATH]");
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
