import { mkdir, rm } from "node:fs/promises";
import { resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";

const repositoryRoot = resolve(import.meta.dirname, "../..");
const tauriE2eRoot = resolve(repositoryRoot, "e2e", "tauri");
const scenarioRoot = resolve(import.meta.dirname, "scenarios");
const requested = process.argv.slice(2).find((arg) => arg.startsWith("scenario="))?.slice("scenario=".length) ?? "mixer";
const scenarios = new Set(["mixer", "settings", "help", "recovery"]);

if (!scenarios.has(requested)) {
  console.error(`Unknown Pilot scenario '${requested}'. Choose: ${[...scenarios].join(", ")}`);
  process.exit(2);
}

function commandAvailable(command, args) {
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    encoding: "utf8",
    shell: false,
    windowsHide: true,
  });
  return result.error ? { ok: false, detail: result.error.message } : { ok: result.status === 0, detail: `${result.stdout ?? ""}${result.stderr ?? ""}`.trim() };
}

const pilot = commandAvailable("tauri-pilot", ["--version"]);
if (!pilot.ok) {
  console.error("Tauri Pilot is unavailable. Install the documented CLI first: cargo install tauri-pilot-cli --locked");
  console.error(`Diagnostic: ${pilot.detail || "tauri-pilot was not found on PATH"}`);
  process.exit(1);
}

const rust = commandAvailable("rustc", ["--version"]);
const rustMatch = rust.detail.match(/rustc (\d+)\.(\d+)\.(\d+)/);
const rustVersion = rustMatch ? rustMatch.slice(1).map(Number) : [0, 0, 0];
if (!rust.ok || rustVersion[0] < 1 || (rustVersion[0] === 1 && rustVersion[1] < 95)) {
  console.error("Tauri Pilot requires Rust 1.95.0+ (edition 2024); refusing to fall back to the production toolchain.");
  console.error(`Diagnostic: ${rust.detail || "rustc was not found"}`);
  process.exit(1);
}

const scenarioPath = resolve(scenarioRoot, `${requested}.toml`);
const runId = `${requested}-${Date.now()}-${process.pid}`;
const outputRoot = resolve(repositoryRoot, "output", "tauri-pilot", runId);
const configDir = resolve(outputRoot, "config");
await mkdir(configDir, { recursive: true });

const command = [
  process.execPath,
  resolve(tauriE2eRoot, "prepare-debug-frontend.mjs"),
  "--",
  process.execPath,
  resolve(tauriE2eRoot, "prepare-debug-capabilities.mjs"),
  "--provider",
  "pilot",
  "--",
  process.execPath,
  resolve(tauriE2eRoot, "run-with-vite-preview.mjs"),
  "--",
  process.execPath,
  resolve(import.meta.dirname, "run-pilot-session.mjs"),
  `--scenario=${requested}`,
  `--scenario-path=${scenarioPath}`,
  `--output-root=${outputRoot}`,
  `--config-dir=${configDir}`,
];

console.log(`Pilot scenario: ${requested}`);
console.log(`Artifacts: ${outputRoot}`);
let code = 1;
try {
  const child = spawn(command[0], command.slice(1), {
    cwd: repositoryRoot,
    env: {
      ...process.env,
      VOLUMECTL_E2E_DEBUG: "1",
      VOLUMECTL_PILOT_SCENARIO: requested,
      VOLUMECTL_CONFIG_DIR: configDir,
    },
    stdio: "inherit",
    windowsHide: true,
  });
  code = await new Promise((resolvePromise, reject) => {
    child.once("error", reject);
    child.once("exit", (exitCode, signal) => resolvePromise(exitCode ?? (signal ? 1 : 0)));
  });
} finally {
  await rm(configDir, { recursive: true, force: true });
}
process.exitCode = code;
