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

const specsBySurface = {
  mixer: ["mixer.e2e.ts"],
  settings: ["settings.e2e.ts"],
  help: ["help.e2e.ts"],
  runtime: ["runtime.e2e.ts"],
};

if (!(requestedSurface === "all" || requestedSurface in specsBySurface)) {
  console.error("Usage: node run-debug-e2e.mjs [--surface mixer|settings|help|runtime|all] [--spec path]");
  process.exit(2);
}

function runSurface(surface, spec) {
  const label = `window-${surface === "runtime" ? "mixer" : surface}`;
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
      env: { ...process.env, VOLUMECTL_VERIFY_SURFACE: label },
      stdio: "inherit",
      windowsHide: true,
    },
  );
  return new Promise((resolvePromise, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolvePromise(code ?? (signal ? 1 : 0)));
  });
}

const surfaces = requestedSurface === "all" ? ["mixer", "runtime", "settings", "help"] : [requestedSurface];
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
