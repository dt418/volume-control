import { access, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";

const repositoryRoot = resolve(import.meta.dirname, "../..");
const args = new Map(process.argv.slice(2).filter((arg) => arg.startsWith("--")).map((arg) => {
  const [key, ...value] = arg.slice(2).split("=");
  return [key, value.join("=")];
}));
const scenario = args.get("scenario") ?? "mixer";
const startupSurface = ["mixer", "settings", "help"].includes(scenario) ? scenario : "mixer";
const startupWindow = `window-${startupSurface}`;
const scenarioPath = args.get("scenario-path");
const outputRoot = resolve(args.get("output-root") ?? resolve(repositoryRoot, "output", "tauri-pilot"));
const configDir = resolve(args.get("config-dir") ?? resolve(outputRoot, "config"));
const binary = process.env.TAURI_E2E_BINARY ?? resolve(repositoryRoot, "target", "debug", process.platform === "win32" ? "VolumeControl.exe" : "VolumeControl");

if (!scenarioPath) throw new Error("Pilot session requires --scenario-path");
await access(scenarioPath);
await mkdir(outputRoot, { recursive: true });

const build = spawn("cargo", [
  "build",
  "-p",
  "volumecontrol-tauri",
  "--no-default-features",
  "--features",
  "e2e-pilot",
], { cwd: repositoryRoot, stdio: "inherit", windowsHide: true });
const buildCode = await new Promise((resolvePromise, reject) => {
  build.once("error", reject);
  build.once("exit", (code, signal) => resolvePromise(code ?? (signal ? 1 : 0)));
});
if (buildCode !== 0) throw new Error(`Pilot debug build failed with exit code ${buildCode}`);
await access(binary);

const app = spawn(binary, [], {
  cwd: repositoryRoot,
  env: {
    ...process.env,
    VOLUMECTL_E2E_DEBUG: "1",
    VITE_TAURI_E2E_DEBUG: "1",
    VOLUMECTL_E2E_PILOT: "1",
    VOLUMECTL_VERIFY_SURFACE: startupWindow,
    VOLUMECTL_CONFIG_DIR: configDir,
    TAURI_PILOT_WINDOW: startupWindow,
    ...(scenario === "recovery" ? { VOLUMECTL_E2E_BOOTSTRAP_FAILURE: "1" } : {}),
  },
  stdio: "inherit",
  windowsHide: true,
});
const appExit = new Promise((resolvePromise) => {
  app.once("error", (error) => {
    console.error(`Pilot app failed to start: ${error.message}`);
    resolvePromise({ code: 1, signal: null });
  });
  app.once("exit", (code, signal) => resolvePromise({ code, signal }));
});

function pilotArgs(command, extra = [], window = startupWindow) {
  return [...(window ? ["--window", window] : []), command, ...extra];
}

function runPilot(command, extra = [], window = startupWindow) {
  const env = { ...process.env };
  if (window) env.TAURI_PILOT_WINDOW = window;
  else delete env.TAURI_PILOT_WINDOW;
  return spawnSync("tauri-pilot", pilotArgs(command, extra, window), {
    cwd: repositoryRoot,
    env,
    encoding: "utf8",
    shell: false,
    windowsHide: true,
  });
}

async function waitForPilot(timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  let detail = "";
  while (Date.now() < deadline) {
    const result = runPilot("ping");
    if (result.status === 0) return;
    detail = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
    const exited = await Promise.race([appExit.then(() => true), new Promise((resolvePromise) => setTimeout(() => resolvePromise(false), 100))]);
    if (exited) break;
  }
  throw new Error(`Tauri Pilot could not connect to the debug app within ${timeoutMs}ms. ${detail}`);
}

async function waitForSurface(selector, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  let detail = "";
  while (Date.now() < deadline) {
    const result = runPilot("wait", ["--selector", selector, "--timeout", "1000"]);
    if (result.status === 0) return;
    detail = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 150));
  }
  throw new Error(`Tauri Pilot surface ${selector} did not become ready within ${timeoutMs}ms. ${detail}`);
}

async function waitForWindow(label, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  let detail = "";
  while (Date.now() < deadline) {
    const result = runPilot("state", [], label);
    if (result.status === 0) return;
    detail = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 150));
  }
  throw new Error(`Tauri Pilot window ${label} did not become available within ${timeoutMs}ms. ${detail}`);
}

async function stopApp() {
  if (!app.pid) return;
  if (process.platform === "win32") {
    await new Promise((resolvePromise) => {
      const killer = spawn("taskkill", ["/pid", String(app.pid), "/t", "/f"], { stdio: "ignore", windowsHide: true });
      killer.once("exit", () => resolvePromise());
      killer.once("error", () => resolvePromise());
    });
  } else {
    app.kill("SIGTERM");
  }
  await Promise.race([appExit, new Promise((resolvePromise) => setTimeout(resolvePromise, 2_000))]);
}

async function cleanupPilotRegistry() {
  if (process.platform !== "win32" || !process.env.LOCALAPPDATA || !app.pid) return;
  const registry = resolve(process.env.LOCALAPPDATA, "tauri-pilot", "instances", "dev.volumecontrol.json");
  try {
    const entry = JSON.parse(await readFile(registry, "utf8"));
    if (entry.pid === app.pid || !isProcessAlive(entry.pid)) {
      await rm(registry, { force: true });
    }
  } catch (error) {
    if (error?.code !== "ENOENT") console.warn(`Pilot registry cleanup skipped: ${error.message}`);
  }
}

function isProcessAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

let exitCode = 1;
try {
  await waitForPilot();
  await waitForWindow(startupWindow);
  await waitForSurface(`[data-surface="${startupSurface}"]`);
  const junit = resolve(outputRoot, `${scenario}.junit.xml`);
  const result = spawnSync("tauri-pilot", ["--window", startupWindow, "run", scenarioPath, "--junit", junit], {
    cwd: repositoryRoot,
    env: { ...process.env, TAURI_PILOT_WINDOW: startupWindow },
    stdio: "inherit",
    shell: false,
    windowsHide: true,
  });
  exitCode = result.status ?? 1;
  const logs = spawnSync("tauri-pilot", ["--window", startupWindow, "logs", "--level", "error"], {
    cwd: repositoryRoot,
    env: { ...process.env, TAURI_PILOT_WINDOW: startupWindow },
    encoding: "utf8",
    shell: false,
    windowsHide: true,
  });
  await writeFile(resolve(outputRoot, `${scenario}.console-errors.log`), `${logs.stdout ?? ""}${logs.stderr ?? ""}`, "utf8");
} finally {
  await stopApp();
  await cleanupPilotRegistry();
}
process.exitCode = exitCode;
