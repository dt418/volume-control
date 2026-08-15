#!/usr/bin/env node

/**
 * Fail-closed contract for the desktop WebDriver provider.
 *
 * This is deliberately a small executable contract rather than a WDIO hook:
 * CI can run it before starting a desktop session, and it never changes
 * E2E_DRIVER_PROVIDER in the parent or child environment.
 */
import { access } from "node:fs/promises";
import { constants } from "node:fs";
import { delimiter, join } from "node:path";
import { createRequire } from "node:module";

const providers = new Set(["embedded", "tauri-driver"]);
const platforms = new Set(["windows", "linux", "macos"]);

function usage(message) {
  if (message) console.error(`Provider contract failed: ${message}`);
  console.error("Usage: node e2e/tauri/test-provider.mjs --provider embedded|tauri-driver --platform windows|linux|macos");
  return 2;
}

function argument(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

async function installedEmbeddedProvider() {
  try {
    const require = createRequire(import.meta.url);
    require.resolve("@wdio/tauri-service", { paths: [import.meta.dirname] });
    return true;
  } catch {
    return false;
  }
}

function providerExecutable(platform) {
  return platform === "windows" ? "tauri-driver.exe" : "tauri-driver";
}

async function executableOnPath(name) {
  const pathEntries = (process.env.PATH ?? "").split(delimiter).filter(Boolean);
  const names = [name];
  if (platformIsWindows() && !name.toLowerCase().endsWith(".exe")) names.push(`${name}.exe`);
  for (const entry of pathEntries) {
    for (const candidate of names) {
      try {
        await access(join(entry, candidate), constants.X_OK);
        return join(entry, candidate);
      } catch {
        // Continue through PATH; availability is a contract failure, not a throw.
      }
    }
  }
  return undefined;
}

function platformIsWindows() {
  return process.platform === "win32";
}

async function main() {
  const provider = argument("--provider");
  const platform = argument("--platform");
  if (!providers.has(provider)) return usage(`unknown provider ${provider ?? "(missing)"}`);
  if (!platforms.has(platform)) return usage(`unknown platform ${platform ?? "(missing)"}`);

  if (provider === "embedded") {
    if (!(await installedEmbeddedProvider())) {
      console.error("Provider contract failed: @wdio/tauri-service is not installed");
      return 1;
    }
    console.log(`Provider available: embedded (${platform})`);
    return 0;
  }

  const executable = providerExecutable(platform);
  const resolved = await executableOnPath(executable);
  if (!resolved) {
    console.error(`Provider contract failed: ${executable} is not available on PATH for ${platform}`);
    return 1;
  }
  console.log(`Provider available: tauri-driver (${platform}) at ${resolved}`);
  return 0;
}

process.exitCode = await main();
