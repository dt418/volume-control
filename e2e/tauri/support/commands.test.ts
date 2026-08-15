import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  assertNoRuntimeErrors,
  collectRuntimeErrors,
  invokeForTest,
  startE2eApp,
  waitForSurface,
  type E2eBrowser,
} from "./commands.ts";

function fakeBrowser(): { browser: E2eBrowser; waits: string[] } {
  const waits: string[] = [];
  const browser: E2eBrowser = {
    $: async (selector) => ({
      waitForDisplayed: async () => {
        waits.push(selector);
      },
    }),
  };
  return { browser, waits };
}

test("waits for a stable surface root and title without fixed sleeps", async () => {
  const { browser, waits } = fakeBrowser();
  await waitForSurface(browser, "mixer", 1_000);
  assert.deepEqual(waits, [
    '[data-surface="mixer"]',
    '[data-surface="mixer"] h1',
  ]);
});

test("invokes deterministic backend setup through browser.tauri.execute", async () => {
  const { browser } = fakeBrowser();
  let received = "";
  browser.tauri = {
    execute: async (script, ...args) => {
      const fn = script as (
        tauri: { core: { invoke: (name: string, payload: Record<string, unknown>) => Promise<unknown> } },
        payload: { command: string; args: Record<string, unknown> },
      ) => Promise<unknown>;
      return fn(
        { core: { invoke: async (name) => { received = name; return { ok: true }; } } },
        args[0] as { command: string; args: Record<string, unknown> },
      );
    },
  };
  assert.deepEqual(await invokeForTest(browser, "get_bootstrap"), { ok: true });
  assert.equal(received, "get_bootstrap");
});

test("requires a connected WDIO session", async () => {
  await assert.rejects(startE2eApp("embedded", undefined), /not connected/);
});

test("rejects non-empty frontend runtime errors", async () => {
  const { browser } = fakeBrowser();
  browser.execute = async () => ({ frontend: ["uncaught frontend failure"], backend: [] });

  assert.deepEqual(await collectRuntimeErrors(browser), {
    frontend: ["uncaught frontend failure"],
    backend: [],
  });
  await assert.rejects(
    assertNoRuntimeErrors(browser),
    /Unexpected frontend runtime errors: uncaught frontend failure/,
  );
});

test("captures backend errors from the WDIO service log file", async () => {
  const logDir = await mkdtemp(join(tmpdir(), "volumecontrol-e2e-backend-log-"));
  const previousLogDir = process.env.TAURI_E2E_LOG_DIR;
  try {
    process.env.TAURI_E2E_LOG_DIR = logDir;
    await mkdir(join(logDir, "service"));
    await writeFile(join(logDir, "service", "tauri-service.log"), [
      "[Tauri:Backend:0] WARN audio i/o error is an allowed warning",
      "[Tauri:Backend:0] ERROR backend failure",
    ].join("\n"));
    const { browser } = fakeBrowser();
    assert.deepEqual(await collectRuntimeErrors(browser), {
      frontend: [],
      backend: ["[Tauri:Backend:0] ERROR backend failure"],
    });
    await assert.rejects(assertNoRuntimeErrors(browser), /Unexpected backend runtime errors:.*backend failure/);
  } finally {
    if (previousLogDir === undefined) delete process.env.TAURI_E2E_LOG_DIR;
    else process.env.TAURI_E2E_LOG_DIR = previousLogDir;
    await rm(logDir, { recursive: true, force: true });
  }
});
