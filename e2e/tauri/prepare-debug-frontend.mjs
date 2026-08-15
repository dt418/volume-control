import { access, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { spawn } from "node:child_process";

const packageRoot = resolve(import.meta.dirname);
const repositoryRoot = resolve(packageRoot, "../..");
const distRoot = resolve(repositoryRoot, "frontend", "dist");
const pluginSource = resolve(packageRoot, "node_modules", "@wdio", "tauri-plugin", "dist-js", "index.js");
const pluginTarget = resolve(distRoot, "tauri-plugin.wdio.js");
const marker = "<!-- volumecontrol:wdio-plugin -->";
const globalTauriBridge = `/* volumecontrol:wdio-tauri-global-bridge */
(() => {
  globalThis.__volumecontrol_e2e_errors ??= [];
  globalThis.addEventListener("error", (event) => {
    globalThis.__volumecontrol_e2e_errors.push(String(event.error?.stack || event.message || event));
  });
  globalThis.addEventListener("unhandledrejection", (event) => {
    globalThis.__volumecontrol_e2e_errors.push(String(event.reason?.stack || event.reason || event));
  });
  const internals = globalThis.__TAURI_INTERNALS__;
  if (!globalThis.__TAURI__ && typeof internals?.invoke === "function") {
    const core = {
      invoke: internals.invoke.bind(internals),
    };
    if (typeof internals.transformCallback === "function") {
      core.transformCallback = internals.transformCallback.bind(internals);
    }
    globalThis.__TAURI__ = { core };
  }
})();
`;

function fail(message) {
  console.error(`ERROR: ${message}`);
  process.exit(2);
}

async function findHtmlFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...await findHtmlFiles(path));
    else if (entry.isFile() && entry.name.endsWith(".html")) files.push(path);
  }
  return files;
}

async function runCommand(command) {
  if (command.length === 0) return;
  await new Promise((resolvePromise, reject) => {
    const child = spawn(command[0], command.slice(1), {
      cwd: repositoryRoot,
      env: { ...process.env, VITE_TAURI_E2E_DEBUG: "1" },
      stdio: "inherit",
    });
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolvePromise();
      else reject(new Error(`debug frontend command failed: code=${code ?? "null"}, signal=${signal ?? "none"}`));
    });
  });
}

const args = process.argv.slice(2);
const checkOnly = args.includes("--check-only");
const separator = args.indexOf("--");
const command = separator >= 0 ? args.slice(separator + 1) : [];

try {
  await access(pluginSource);
  const htmlFiles = await findHtmlFiles(distRoot);
  if (htmlFiles.length === 0) throw new Error(`no frontend HTML files found under ${distRoot}`);
  try {
    await stat(pluginTarget);
    throw new Error(`refusing to reuse pre-existing debug bridge ${pluginTarget}`);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  const originals = new Map();
  for (const file of htmlFiles) {
    const content = await readFile(file, "utf8");
    if (content.includes(marker)) throw new Error(`debug bridge marker already present in ${file}`);
    if (!content.includes("</head>")) throw new Error(`missing </head> in ${file}`);
    originals.set(file, content);
  }

  if (checkOnly) {
    console.log(`Debug frontend sources OK: ${htmlFiles.length} HTML files; no tracked dist mutation`);
    process.exit(0);
  }

  const pluginContents = await readFile(pluginSource, "utf8");
  await writeFile(pluginTarget, `${globalTauriBridge}\n${pluginContents}`, "utf8");
  for (const [file, content] of originals) {
    const pluginHref = relative(dirname(file), pluginTarget).replaceAll("\\", "/");
    await writeFile(file, content.replace("</head>", `${marker}\n<script type="module" src="${pluginHref}"></script>\n</head>`));
  }

  try {
    await runCommand(command);
  } finally {
    for (const [file, content] of originals) await writeFile(file, content);
    await rm(pluginTarget, { force: true });
  }
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}
