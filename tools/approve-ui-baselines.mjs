import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { basename, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// Test runs only create candidates; review records identify a human or explicitly delegated agent.
const args = process.argv.slice(2);
const directory = resolve(args[args.indexOf("--from") + 1] ?? "");
const reviewer = args[args.indexOf("--reviewer") + 1];
const reviewMode = args.includes("--delegated") ? "user-delegated-agent" : "human";
const authorization = args.includes("--authorization")
  ? args[args.indexOf("--authorization") + 1]
  : null;
try {
  assert(
    args.includes("--reviewed") &&
      args.includes("--from") &&
      args.includes("--reviewer") &&
      reviewer?.trim(),
    "Usage after review: approve-ui-baselines.mjs --from RUN_DIR --reviewer NAME --reviewed [--delegated --authorization TEXT]",
  );
  if (reviewMode === "user-delegated-agent")
    assert(
      authorization?.trim(),
      "Delegated review requires the user's explicit authorization text",
    );
  const root = fileURLToPath(new URL("..", import.meta.url));
  const functional = JSON.parse(readFileSync(join(directory, "functional-results.json"), "utf8"));
  const cold = JSON.parse(readFileSync(join(directory, "cold-start-results.json"), "utf8"));
  assert(
    functional.cases.length >= 7 && functional.cases.every((c) => c.passed),
    "Functional UI acceptance has failures",
  );
  assert(cold.length === 2 && cold.every((c) => c.passed), "Cold-start acceptance has failures");
  const visuals = JSON.parse(readFileSync(join(directory, "visual-results.json"), "utf8"));
  assert.equal(visuals.length, 22, "Expected the complete reviewed screenshot set");
  const target = join(root, "tests/desktop/baselines");
  mkdirSync(target, { recursive: true });
  const manifestPath = join(target, "review.json");
  const manifest = existsSync(manifestPath)
    ? JSON.parse(readFileSync(manifestPath, "utf8"))
    : { schemaVersion: 1, images: {} };
  for (const item of visuals) {
    assert(/^[a-z0-9-]+$/.test(item.name));
    const source = realpathSync(join(directory, `${item.name}.png`));
    const withinRun = relative(realpathSync(directory), source);
    assert(
      !isAbsolute(withinRun) && !withinRun.startsWith(".."),
      "Screenshot escapes its test run",
    );
    const sha256 = createHash("sha256").update(readFileSync(source)).digest("hex");
    copyFileSync(source, join(target, `${item.name}.png`));
    manifest.images[item.name] = {
      sha256,
      reviewer,
      reviewMode,
      authorization,
      reviewedAt: new Date().toISOString(),
      sourceRun: basename(directory),
      environment: item.environment,
    };
  }
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`Recorded ${visuals.length} reviewed baselines. Rerun pnpm test:ui to compare them.`);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
