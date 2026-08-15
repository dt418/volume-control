import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import type { Capabilities, Options } from "@wdio/types";
import { createAppFixture } from "./support/app-fixture.ts";

const packageRoot = resolve(import.meta.dirname);
const repositoryRoot = resolve(packageRoot, "../..");
const binaryName = process.platform === "win32" ? "VolumeControl.exe" : "VolumeControl";
const fixture = await createAppFixture({ binary: process.env.TAURI_E2E_BINARY });
const appBinaryPath = process.env.TAURI_E2E_BINARY ?? fixture.binary ?? resolve(repositoryRoot, "target", "debug", binaryName);
const outputRoot = resolve(
  process.env.TAURI_E2E_OUTPUT ?? fixture.outputDir,
);
const driverProvider = process.env.E2E_DRIVER_PROVIDER === "tauri-driver" ? "tauri-driver" : "embedded";
const verifySurface = process.env.VOLUMECTL_VERIFY_SURFACE ?? "window-mixer";

await mkdir(outputRoot, { recursive: true });
process.env.VOLUMECTL_E2E_DEBUG = "1";
process.env.VOLUMECTL_CONFIG_DIR = fixture.configDir;
process.env.VOLUMECTL_VERIFY_SURFACE = verifySurface;
process.env.TAURI_E2E_OUTPUT = outputRoot;

export const config: Options.Testrunner & Capabilities.WithRequestedTestrunnerCapabilities = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  logLevel: "warn",
  framework: "mocha",
  services: [
    [
      "@wdio/tauri-service",
      {
        appBinaryPath,
        driverProvider,
        captureBackendLogs: true,
        captureFrontendLogs: true,
        startTimeout: 60_000,
        windowLabel: verifySurface,
      },
    ],
  ],
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": {
        application: appBinaryPath,
      },
    } as unknown as Capabilities.RequestedStandaloneCapabilities,
  ],
  reporters: [["spec", { stdout: true }]],
  mochaOpts: {
    ui: "bdd",
    timeout: 60_000,
  },
  outputDir: outputRoot,
  baseUrl: "http://localhost:4445",
  onComplete: async () => {
    await fixture.cleanup();
  },
};

export default config;
