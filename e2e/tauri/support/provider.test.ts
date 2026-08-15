import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const testDirectory = dirname(fileURLToPath(import.meta.url));
const providerScript = resolve(testDirectory, "..", "test-provider.mjs");

interface ProviderResult {
  code: number | null;
  stdout: string;
  stderr: string;
}

function runProvider(args: string[], overrides: NodeJS.ProcessEnv = {}): Promise<ProviderResult> {
  return new Promise((resolveResult, reject) => {
    const child = spawn(process.execPath, [providerScript, ...args], {
      env: { ...process.env, ...overrides },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk: Buffer) => { stdout += chunk.toString(); });
    child.stderr.on("data", (chunk: Buffer) => { stderr += chunk.toString(); });
    child.on("error", reject);
    child.on("close", (code) => resolveResult({ code, stdout, stderr }));
  });
}

test("provider contract accepts an installed embedded provider", async () => {
  const result = await runProvider(["--provider", "embedded", "--platform", "windows"]);
  assert.equal(result.code, 0, result.stderr);
  assert.match(result.stdout, /Provider available: embedded/);
});

test("provider contract rejects tauri-driver when the executable is absent", async () => {
  const emptyPath = await mkdtemp("volumecontrol-provider-path-");
  try {
    const result = await runProvider(
      ["--provider", "tauri-driver", "--platform", "windows"],
      { PATH: emptyPath, Path: emptyPath },
    );
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /tauri-driver\.exe is not available/);
  } finally {
    await rm(emptyPath, { recursive: true, force: true });
  }
});

test("provider contract rejects unknown providers", async () => {
  const result = await runProvider(["--provider", "unknown", "--platform", "windows"]);
  assert.notEqual(result.code, 0);
  assert.match(result.stderr, /unknown provider/);
});

test("provider contract does not rewrite E2E_DRIVER_PROVIDER", async () => {
  const previous = process.env.E2E_DRIVER_PROVIDER;
  const result = await runProvider(
    ["--provider", "embedded", "--platform", "windows"],
    { E2E_DRIVER_PROVIDER: "tauri-driver" },
  );
  assert.equal(result.code, 0, result.stderr);
  assert.equal(process.env.E2E_DRIVER_PROVIDER, previous);
});
