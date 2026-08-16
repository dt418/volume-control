import { spawn } from "node:child_process";
import { resolve } from "node:path";

const packageRoot = resolve(import.meta.dirname);
const repositoryRoot = resolve(packageRoot, "../..");
const prepareFrontend = resolve(packageRoot, "prepare-debug-frontend.mjs");
const prepareCapabilities = resolve(packageRoot, "prepare-debug-capabilities.mjs");
const previewRunner = resolve(packageRoot, "run-with-vite-preview.mjs");
const wdioCli = resolve(packageRoot, "node_modules", "@wdio", "cli", "bin", "wdio.js");
const wdioConfig = resolve(packageRoot, "wdio.conf.ts");

const args = process.argv.slice(2);
const surfaceIndex = args.indexOf("--surface");
const requestedSurface = surfaceIndex >= 0 ? args[surfaceIndex + 1] : "all";
const specIndex = args.indexOf("--spec");
const requestedSpec = specIndex >= 0 ? args[specIndex + 1] : undefined;
// Real audio by default: the app talks to the actual OS endpoint, so local
// runs exercise the true backend. Hosted CI (and machines without a default
// endpoint) pass --virtual-audio for deterministic assertions.
const virtualAudio = args.includes("--virtual-audio");

const specsBySurface = {
  mixer: ["mixer.e2e.ts"],
  settings: ["settings.e2e.ts"],
  help: ["help.e2e.ts"],
  overlay: ["overlay.e2e.ts"],
  runtime: ["runtime.e2e.ts"],
  recovery: ["recovery.e2e.ts"],
  windows: ["windows.e2e.ts"],
};

if (!(requestedSurface === "all" || requestedSurface in specsBySurface)) {
  console.error("Usage: node run-debug-e2e.mjs [--surface mixer|runtime|windows|recovery|settings|help|overlay|all] [--spec path] [--virtual-audio]");
  process.exit(2);
}

function runSurface(surface, spec) {
  const startupSurface = surface === "settings" || surface === "help" || surface === "overlay" ? surface : "mixer";
  const label = `window-${startupSurface}`;
  const specPath = resolve(packageRoot, "specs", spec);
  const child = spawn(
    process.execPath,
    [
      prepareFrontend,
      "--",
      process.execPath,
      prepareCapabilities,
      "--provider",
      "wdio",
      "--",
      process.execPath,
      previewRunner,
      "--",
      process.execPath,
      wdioCli,
      "run",
      wdioConfig,
      "--spec",
      specPath,
    ],
    {
      cwd: repositoryRoot,
      env: {
        ...process.env,
        VOLUMECTL_VERIFY_SURFACE: label,
        VOLUMECTL_E2E_REQUESTED_SURFACE: surface,
        // Opt-in virtual backend: hosted CI runners do not guarantee a
        // physical default output. Local runs use the REAL audio backend so
        // the E2E exercises actual device volume/mute round-trips.
        ...(virtualAudio ? { VOLUMECTL_E2E_AUDIO: "virtual" } : {}),
        ...(surface === "recovery" ? { VOLUMECTL_E2E_BOOTSTRAP_FAILURE: "1" } : {}),
      },
      stdio: "inherit",
      windowsHide: true,
    },
  );
  return new Promise((resolvePromise, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolvePromise(code ?? (signal ? 1 : 0)));
  });
}

// The webview overlay is macOS/Linux-only (Windows uses the native HUD and
// never opens window-overlay), so the "all" matrix skips it on win32.
const coreSurfaces = ["mixer", "runtime", "windows", "recovery", "settings", "help"];
const surfaces =
  requestedSurface === "all"
    ? [...coreSurfaces, ...(process.platform === "win32" ? [] : ["overlay"])]
    : [requestedSurface];
let exitCode = 0;
for (const surface of surfaces) {
  const spec = requestedSpec ?? specsBySurface[surface][0];
  const code = await runSurface(surface, spec);
  if (code !== 0) {
    exitCode = code;
    break;
  }
}

process.exitCode = exitCode;
