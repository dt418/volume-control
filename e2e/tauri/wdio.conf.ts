import { mkdir, readFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import type { Capabilities, Options } from "@wdio/types";
import { createAppFixture } from "./support/app-fixture.ts";
import { sanitizeArtifactName, timingReport, writeE2eManifest, type E2eManifest } from "./support/artifacts.ts";

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
await mkdir(join(outputRoot, "junit"), { recursive: true });
await mkdir(join(outputRoot, "logs"), { recursive: true });
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
        backendLogLevel: "info",
        frontendLogLevel: "info",
        logDir: join(outputRoot, "logs"),
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
  reporters: [
    ["spec", { stdout: true }],
    ["junit", {
      outputDir: join(outputRoot, "junit"),
      outputFileFormat: () => `${sanitizeArtifactName(specName())}.xml`,
    }],
  ],
  mochaOpts: {
    ui: "bdd",
    timeout: 60_000,
  },
  outputDir: outputRoot,
  baseUrl: "http://localhost:4445",
  onComplete: async (exitCode) => {
    try {
      timingReport(outputRoot);
      const spec = specName();
      let existing: E2eManifest = { results: [] };
      try {
        existing = JSON.parse(await readFile(join(outputRoot, "manifest.json"), "utf8")) as E2eManifest;
      } catch {
        // The first isolated surface session creates the manifest.
      }
      const surface = verifySurface.replace(/^window-/, "");
      const result = { spec, surface, status: exitCode === 0 ? "passed" as const : "failed" as const };
      await writeE2eManifest(outputRoot, {
        ...existing,
        results: [...(existing.results ?? []).filter((entry) => entry.spec !== spec || entry.surface !== surface), result],
      });
    } finally {
      await fixture.cleanup();
    }
  },
};

function specName(): string {
  const specArgument = process.argv.find((argument) => argument.endsWith(".e2e.ts"));
  return specArgument ? basename(specArgument) : `${sanitizeArtifactName(verifySurface)}.e2e.ts`;
}

export default config;
