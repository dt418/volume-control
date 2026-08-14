import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import type { Options } from "@wdio/types";

const packageRoot = resolve(import.meta.dirname);
const repositoryRoot = resolve(packageRoot, "../..");
const binaryName = process.platform === "win32" ? "VolumeControl.exe" : "VolumeControl";
const appBinaryPath = process.env.TAURI_E2E_BINARY ?? resolve(repositoryRoot, "target", "debug", binaryName);
const outputRoot = resolve(
  process.env.TAURI_E2E_OUTPUT ?? resolve(repositoryRoot, "output", "tauri-e2e"),
);
const driverProvider = process.env.E2E_DRIVER_PROVIDER === "tauri-driver" ? "tauri-driver" : "embedded";

await mkdir(outputRoot, { recursive: true });

export const config: Options.Testrunner = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  logLevel: "warn",
  framework: "mocha",
  services: [
    [
      "tauri",
      {
        appBinaryPath,
        driverProvider,
      },
    ],
  ],
  reporters: [["spec", { stdout: true }]],
  mochaOpts: {
    ui: "bdd",
    timeout: 60_000,
  },
  outputDir: outputRoot,
  baseUrl: "tauri://localhost",
};

export default config;
