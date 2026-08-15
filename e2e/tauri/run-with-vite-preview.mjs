import { spawn } from "node:child_process";
import { resolve } from "node:path";

const packageRoot = resolve(import.meta.dirname);
const repositoryRoot = resolve(packageRoot, "../..");
const viteCli = resolve(repositoryRoot, "frontend", "node_modules", "vite", "bin", "vite.js");
const args = process.argv.slice(2);
const separator = args.indexOf("--");
const command = separator >= 0 ? args.slice(separator + 1) : [];

if (command.length === 0) {
  console.error("Usage: node run-with-vite-preview.mjs -- command args...");
  process.exit(2);
}

function spawnCommand(commandArgs, options = {}) {
  return spawn(commandArgs[0], commandArgs.slice(1), {
    cwd: options.cwd ?? repositoryRoot,
    env: { ...process.env, ...options.env },
    stdio: "inherit",
    windowsHide: true,
  });
}

async function waitForPreview(url, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = new Error(`preview returned HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 150));
  }
  throw new Error(`Vite preview did not become ready: ${lastError instanceof Error ? lastError.message : String(lastError)}`);
}

async function stopProcess(child) {
  if (!child.pid) return;
  if (process.platform === "win32") {
    await new Promise((resolvePromise) => {
      const killer = spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
        stdio: "ignore",
        windowsHide: true,
      });
      killer.once("error", () => resolvePromise());
      killer.once("exit", () => resolvePromise());
    });
  } else {
    child.kill("SIGTERM");
  }
  await Promise.race([
    previewExit,
    new Promise((resolvePromise) => setTimeout(resolvePromise, 2_000)),
  ]);
}

const preview = spawnCommand([
  process.execPath,
  viteCli,
  "preview",
  "--host",
  "127.0.0.1",
  "--port",
  "1420",
  "--strictPort",
], { cwd: resolve(repositoryRoot, "frontend") });

const previewExit = new Promise((resolvePromise, reject) => {
  preview.once("error", reject);
  preview.once("exit", (code, signal) => resolvePromise({ code, signal }));
});

let exitCode = 1;
try {
  await Promise.race([
    waitForPreview("http://127.0.0.1:1420/src/mixer/index.html"),
    previewExit.then(({ code, signal }) => {
      throw new Error(`Vite preview exited before readiness: code=${code ?? "null"}, signal=${signal ?? "none"}`);
    }),
  ]);

  const child = spawnCommand(command, { VITE_TAURI_E2E_DEBUG: "1" });
  exitCode = await new Promise((resolvePromise, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolvePromise(code ?? (signal ? 1 : 0)));
  });
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
} finally {
  await stopProcess(preview);
}

process.exitCode = exitCode;
