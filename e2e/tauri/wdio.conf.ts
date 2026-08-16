import { mkdir, readFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import type { Capabilities, Options } from "@wdio/types";
import { createAppFixture } from "./support/app-fixture.ts";
import {
  assertConfiguredTimingBudgets,
  sanitizeArtifactName,
  timingReport,
  writeE2eManifest,
  type E2eManifest,
} from "./support/artifacts.ts";

const packageRoot = resolve(import.meta.dirname);
const repositoryRoot = resolve(packageRoot, "../..");
const configuredProvider = process.env.E2E_DRIVER_PROVIDER;
if (configuredProvider !== "embedded" && configuredProvider !== "tauri-driver") {
  throw new Error(
    "E2E_DRIVER_PROVIDER must be explicitly set to embedded or tauri-driver; refusing an implicit provider",
  );
}
const driverProvider = configuredProvider;
const binaryName = process.platform === "win32" ? "VolumeControl.exe" : "VolumeControl";
const fixture = await createAppFixture({ binary: process.env.TAURI_E2E_BINARY });
const appBinaryPath = process.env.TAURI_E2E_BINARY ?? fixture.binary ?? resolve(repositoryRoot, "target", "debug", binaryName);
const outputRoot = resolve(
  process.env.TAURI_E2E_OUTPUT ?? fixture.outputDir,
);
const verifySurface = process.env.VOLUMECTL_VERIFY_SURFACE ?? "window-mixer";
const requestedSurface = process.env.VOLUMECTL_E2E_REQUESTED_SURFACE ?? verifySurface.replace(/^window-/, "");
const runId = process.env.TAURI_E2E_RUN_ID ?? ["wdio", Date.now(), Math.random().toString(36).slice(2, 10)].join("-");

await mkdir(outputRoot, { recursive: true });
await mkdir(join(outputRoot, "junit"), { recursive: true });
await mkdir(join(outputRoot, "logs"), { recursive: true });
// Isolate WebView2 user data per surface run. The embedded service spawns a
// fresh app per surface; without isolation, orphaned WebView2 renderer
// processes from force-killed previous runs lock the shared user-data folder
// and the next app's webview breaks mid-spec ("No window could be found" on
// execute/sync, flaky only after several consecutive app restarts).
if (process.platform === "win32") {
  process.env.WEBVIEW2_USER_DATA_FOLDER = join(outputRoot, `webview2-${process.pid}`);
}
process.env.VOLUMECTL_E2E_DEBUG = "1";
process.env.VOLUMECTL_CONFIG_DIR = fixture.configDir;
process.env.VOLUMECTL_VERIFY_SURFACE = verifySurface;
process.env.TAURI_E2E_OUTPUT = outputRoot;
process.env.TAURI_E2E_RUN_ID = runId;
process.env.TAURI_E2E_LOG_DIR = join(outputRoot, "logs");

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
  outputDir: join(outputRoot, "logs"),
  baseUrl: "http://localhost:4445",
  onComplete: async (exitCode) => {
    try {
      timingReport(outputRoot);
      if (exitCode === 0) assertConfiguredTimingBudgets();
      const spec = specName();
      let existing: E2eManifest = { results: [] };
      try {
        const candidate = JSON.parse(await readFile(join(outputRoot, "manifest.json"), "utf8")) as E2eManifest;
        if (candidate.runId === runId) existing = candidate;
      } catch {
        // The first isolated surface session creates the manifest.
      }
      const result = { spec, surface: requestedSurface, status: exitCode === 0 ? "passed" as const : "failed" as const };
      await writeE2eManifest(outputRoot, {
        ...existing,
        runId,
        platform: process.platform,
        provider: driverProvider,
        results: [...(existing.results ?? []).filter((entry) => entry.spec !== spec || entry.surface !== requestedSurface), result],
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
