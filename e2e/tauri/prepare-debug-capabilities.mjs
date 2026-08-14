import { access, copyFile, mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { spawn } from "node:child_process";

const repositoryRoot = resolve(import.meta.dirname, "../..");
const sourceRoot = resolve(import.meta.dirname, "capabilities");
const targetRoot = resolve(repositoryRoot, "src-tauri", "capabilities");
const tauriConfigPath = resolve(repositoryRoot, "src-tauri", "tauri.conf.json");

function usage(message) {
  if (message) console.error(`ERROR: ${message}`);
  console.error("Usage: node prepare-debug-capabilities.mjs --provider <wdio|pilot> [--check-only] [-- command args...]");
  process.exitCode = 2;
}

const args = process.argv.slice(2);
const providerIndex = args.indexOf("--provider");
const provider = providerIndex >= 0 ? args[providerIndex + 1] : undefined;
const checkOnly = args.includes("--check-only");
const separator = args.indexOf("--");
const command = separator >= 0 ? args.slice(separator + 1) : [];

if (provider !== "wdio" && provider !== "pilot") {
  usage("provider must be wdio or pilot");
  process.exit(2);
}

const capabilityName = `e2e-${provider}.json`;
const sourcePath = resolve(sourceRoot, capabilityName);
const targetPath = resolve(targetRoot, capabilityName);

async function assertSource() {
  await access(sourcePath);
  const parsed = JSON.parse(await readFile(sourcePath, "utf8"));
  const expectedPermission = provider === "wdio" ? "wdio:default" : "pilot:default";
  if (!parsed.permissions?.includes(expectedPermission)) {
    throw new Error(`${capabilityName} is missing ${expectedPermission}`);
  }
}

async function runCommand() {
  if (command.length === 0) return;
  await new Promise((resolvePromise, reject) => {
    const child = spawn(command[0], command.slice(1), {
      cwd: repositoryRoot,
      env: { ...process.env, VOLUMECTL_E2E_DEBUG: "1" },
      stdio: "inherit",
    });
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolvePromise();
      else reject(new Error(`debug command failed: code=${code ?? "null"}, signal=${signal ?? "none"}`));
    });
  });
}

async function enableDebugGlobalTauri(originalConfig) {
  const config = JSON.parse(originalConfig);
  config.app = { ...(config.app ?? {}), withGlobalTauri: true };
  await writeFile(tauriConfigPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");
}

await assertSource();
if (checkOnly) {
  try {
    await stat(targetPath);
    throw new Error(`${targetPath} already exists; check-only refuses to touch capabilities`);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  console.log(`Capability source OK: ${sourcePath}`);
  process.exit(0);
}

await mkdir(dirname(targetPath), { recursive: true });
const originalTauriConfig = await readFile(tauriConfigPath, "utf8");
try {
  await stat(targetPath);
  throw new Error(`Refusing to overwrite pre-existing capability ${targetPath}`);
} catch (error) {
  if (error?.code !== "ENOENT") throw error;
}

try {
  await copyFile(sourcePath, targetPath);
  await enableDebugGlobalTauri(originalTauriConfig);
  await runCommand();
} finally {
  await writeFile(tauriConfigPath, originalTauriConfig, "utf8");
  await rm(targetPath, { force: true });
}
