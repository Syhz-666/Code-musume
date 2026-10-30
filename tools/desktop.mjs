import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const pnpm =
  process.env.npm_execpath ?? join(process.env.APPDATA ?? "", "npm/node_modules/pnpm/bin/pnpm.cjs");
function run(command, args, env = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    env: { ...process.env, ...env },
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status ?? result.signal})`);
}
const pm = (args, env) => run(process.execPath, [pnpm, ...args], env);
const action = process.argv[2];
try {
  if (action === "replay" || action === "verify")
    run("cargo", [
      "test",
      "-p",
      "code-musume-core",
      "--test",
      "fixture_replay",
      "--",
      "--nocapture",
    ]);
  if (action === "verify") {
    pm(["verify"]);
    pm(["test:tools"]);
    pm(["build"], { VITE_E2E: "0" });
    const verificationTarget = join(root, "target/verification");
    run("cargo", ["build", "--workspace"], { CARGO_TARGET_DIR: verificationTarget });
    run("powershell", [
      "-NoProfile",
      "-File",
      "tools/smoke.ps1",
      "-BinDir",
      join(verificationTarget, "debug"),
    ]);
    for (const file of readdirSync(join(root, "apps/pet/dist/assets"))) {
      if (
        file.endsWith(".js") &&
        /wdioTauri|WDIO Tauri Plugin|e2e_advance_clock/.test(
          readFileSync(join(root, "apps/pet/dist/assets", file), "utf8"),
        )
      )
        throw new Error("Production frontend contains test bootstrap");
    }
    const tree = spawnSync("cargo", ["tree", "-p", "pet", "--edges", "normal"], {
      cwd: root,
      encoding: "utf8",
      windowsHide: true,
    });
    if (tree.status !== 0 || tree.stdout.includes("tauri-plugin-wdio"))
      throw new Error("Production build contains desktop-test plugins");
  }
  if (action === "ui" || action === "verify") {
    if (process.platform !== "win32")
      throw new Error("Desktop UI acceptance requires Windows and WebView2");
    if (!existsSync(join(root, "tests/desktop/node_modules/@wdio/cli")))
      pm([
        "--dir",
        "tests/desktop",
        "install",
        "--frozen-lockfile",
        "--registry=https://registry.npmjs.org",
      ]);
    const runDir = resolve(
      process.env.CODE_MUSUME_UI_RUN_DIR ??
        join(root, ".test-artifacts", `ui-${new Date().toISOString().replaceAll(/[:.]/g, "-")}`),
    );
    mkdirSync(join(runDir, "config"), { recursive: true });
    if (!existsSync(join(runDir, "config/config.json")))
      writeFileSync(
        join(runDir, "config/config.json"),
        JSON.stringify({
          schemaVersion: 1,
          reducedMotion: true,
          fullscreenHide: false,
          autostart: false,
        }),
      );
    const env = {
      CODE_MUSUME_UI_RUN_DIR: runDir,
      CODE_MUSUME_UI_REVIEW: process.argv.includes("--review-candidates") ? "1" : "0",
      CODE_MUSUME_UI_SYSTEM_MOTION: process.argv.includes("--system-reduced-motion")
        ? "reduce"
        : "no-preference",
      CARGO_TARGET_DIR: join(root, "target/e2e"),
      VITE_E2E: "1",
    };
    const revision = spawnSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
      windowsHide: true,
    });
    const dirty = spawnSync("git", ["diff", "--quiet", "HEAD"], { cwd: root, windowsHide: true });
    env.CODE_MUSUME_SOURCE_REVISION =
      process.env.GITHUB_SHA ?? (revision.status === 0 ? revision.stdout.trim() : "unavailable");
    env.CODE_MUSUME_SOURCE_DIRTY = String(dirty.status === 1);
    if (!process.argv.includes("--skip-build")) {
      run("cargo", ["build", "-p", "code-musume-hook"]);
      pm(
        [
          "--filter",
          "@code-musume/pet",
          "tauri",
          "build",
          "--debug",
          "--no-bundle",
          "--features",
          "e2e",
          "--config",
          join(root, "apps/pet/src-tauri/tauri.e2e.conf.json"),
        ],
        env,
      );
    }
    let uiFailure;
    try {
      pm(["--dir", "tests/desktop", "test"], env);
    } catch (error) {
      uiFailure = error;
    }
    if (env.CODE_MUSUME_UI_SYSTEM_MOTION !== "reduce")
      run(process.execPath, ["tools/desktop-cold-start.mjs"], env);
    if (uiFailure) throw uiFailure;
  }
  if (!["replay", "ui", "verify"].includes(action))
    throw new Error(
      "Usage: desktop.mjs replay|ui|verify [--skip-build] [--review-candidates] [--system-reduced-motion]",
    );
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
