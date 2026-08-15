import { access } from "node:fs/promises";
import { createAppFixture, type AppFixture } from "./app-fixture.ts";
import { selectors, type SurfaceName } from "./selectors.ts";

export type DriverProvider = "embedded" | "tauri-driver";

export interface SurfaceElement {
  waitForDisplayed: (options?: { timeout?: number }) => Promise<void>;
}

export interface E2eBrowser {
  $: (selector: string) => Promise<SurfaceElement>;
  execute?: (script: unknown, ...args: unknown[]) => Promise<unknown>;
  getLogs?: (type?: string) => Promise<unknown[]>;
  refresh?: () => Promise<void>;
  tauri?: {
    execute: (script: unknown, ...args: unknown[]) => Promise<unknown>;
    switchWindow?: (label: string) => Promise<void>;
    listWindows?: () => Promise<string[]>;
    mock?: (command: string) => Promise<{ mockRejectedValue: (error: unknown) => Promise<unknown> }>;
    restoreAllMocks?: (commandPrefix?: string) => Promise<unknown>;
    getBackendLogs?: () => Promise<unknown>;
  };
}

/**
 * WDIO's service creates the connected session before a spec starts. This
 * helper makes that lifecycle explicit and fails clearly when a spec is run
 * outside the configured runner.
 */
export async function startE2eApp(
  provider: DriverProvider,
  browser: E2eBrowser | undefined = (globalThis as { browser?: E2eBrowser }).browser,
): Promise<E2eBrowser> {
  if (provider !== "embedded" && provider !== "tauri-driver") {
    throw new Error(`Unsupported E2E driver provider: ${provider}`);
  }
  if (!browser) {
    throw new Error("WebDriver session is not connected; run through wdio.conf.ts");
  }
  return browser;
}

export async function waitForSurface(
  browser: E2eBrowser,
  surface: SurfaceName,
  timeout = 60_000,
): Promise<SurfaceElement> {
  const root = await browser.$(selectors.surfaceRoot(surface));
  await root.waitForDisplayed({ timeout });
  const title = await browser.$(selectors.surfaceTitle(surface));
  await title.waitForDisplayed({ timeout });
  return root;
}

export async function invokeForTest(
  browser: E2eBrowser,
  command: string,
  args: Record<string, unknown> = {},
): Promise<unknown> {
  if (!browser.tauri?.execute) {
    throw new Error("Tauri WDIO plugin is unavailable; deterministic IPC helpers require e2e-wdio");
  }
  return browser.tauri.execute(
    (tauri: { core: { invoke: (name: string, payload: Record<string, unknown>) => Promise<unknown> } },
      payload: { command: string; args: Record<string, unknown> }) =>
      tauri.core.invoke(payload.command, payload.args),
    { command, args },
  );
}

function asMessages(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((entry) => {
      if (typeof entry === "string") return entry;
      if (!entry || typeof entry !== "object") return String(entry);
      const record = entry as { message?: unknown; text?: unknown; args?: unknown };
      if (typeof record.message === "string") return record.message;
      if (typeof record.text === "string") return record.text;
      return JSON.stringify(record.args ?? record);
    })
    .filter((message): message is string => message.length > 0);
}

function isErrorLog(entry: unknown): boolean {
  if (!entry || typeof entry !== "object") return false;
  const record = entry as { level?: unknown; severity?: unknown };
  return record.level === "error" || record.level === "SEVERE" || record.severity === 3 || record.severity === "error";
}

export async function collectRuntimeErrors(browser: E2eBrowser): Promise<{ frontend: string[]; backend: string[] }> {
  let captured: { frontend?: unknown; backend?: unknown } = {};
  if (browser.execute) {
    try {
      const value = await browser.execute(() => {
        const win = window as Window & {
          __volumecontrol_e2e_errors?: unknown[];
          __volumecontrol_e2e_backend_errors?: unknown[];
        };
        return {
          frontend: win.__volumecontrol_e2e_errors ?? [],
          backend: win.__volumecontrol_e2e_backend_errors ?? [],
        };
      });
      if (value && typeof value === "object") captured = value as typeof captured;
    } catch (error) {
      captured.frontend = [error instanceof Error ? error.message : String(error)];
    }
  }

  let browserLogs: unknown[] = [];
  if (browser.getLogs) {
    try {
      browserLogs = await browser.getLogs("browser");
    } catch {
      // A provider without the WebDriver log endpoint still has the in-page
      // error buffer above; do not turn a diagnostic API limitation into a
      // synthetic application error.
    }
  }
  const frontend = [...asMessages(captured.frontend), ...asMessages(browserLogs.filter(isErrorLog))];

  let backend: unknown = captured.backend;
  if (browser.tauri?.getBackendLogs) {
    try {
      backend = await browser.tauri.getBackendLogs();
    } catch (error) {
      backend = [error instanceof Error ? error.message : String(error)];
    }
  }
  return {
    frontend: [...new Set(frontend)],
    backend: [...new Set(asMessages(backend))],
  };
}

const allowedDegradedMessages = [
  /audio backend unavailable/i,
  /global hotkeys? unavailable/i,
  /degraded hotkeys?/i,
];

function isAllowedDegradedMessage(message: string): boolean {
  return allowedDegradedMessages.some((pattern) => pattern.test(message));
}

export async function assertNoRuntimeErrors(browser: E2eBrowser): Promise<void> {
  const errors = await collectRuntimeErrors(browser);
  const frontend = errors.frontend.filter((message) => !isAllowedDegradedMessage(message));
  const backend = errors.backend.filter((message) => !isAllowedDegradedMessage(message));
  if (frontend.length > 0) {
    throw new Error(`Unexpected frontend runtime errors: ${frontend.join("; ")}`);
  }
  if (backend.length > 0) {
    throw new Error(`Unexpected backend runtime errors: ${backend.join("; ")}`);
  }
}

export async function mockBootstrapFailure(browser: E2eBrowser): Promise<() => Promise<void>> {
  const mock = await browser.tauri?.mock?.("get_bootstrap");
  if (!mock) throw new Error("Tauri WDIO mock API is unavailable; run with e2e-wdio");
  await mock.mockRejectedValue(new Error("E2E bootstrap failure"));
  return async () => {
    await browser.tauri?.restoreAllMocks?.("get_bootstrap");
  };
}

export async function listOwnedWindows(browser: E2eBrowser): Promise<string[]> {
  const labels = await browser.tauri?.listWindows?.();
  if (!labels) throw new Error("Tauri WDIO window API is unavailable; run with e2e-wdio");
  return labels.filter((label) => /^window-(mixer|settings|help)$/.test(label));
}

export async function openSurface(browser: E2eBrowser, surface: SurfaceName): Promise<SurfaceElement> {
  const label = `window-${surface}`;
  if (process.env.VOLUMECTL_VERIFY_SURFACE === label) {
    // Surface-specific runs start with the target WebView already created by
    // the debug verification hook; avoid reopening it through WebDriver.
  } else if (surface !== "mixer" && browser.execute) {
    // Opening a second WebView from an embedded WebDriver direct-eval callback
    // can block the originating WebView's event loop. Use the same frontend
    // IPC path as a real click for Settings/Help, then switch native context.
    await browser.execute(
      (target: string) => {
        const internals = (window as Window & {
          __TAURI_INTERNALS__?: { invoke?: (command: string, args: Record<string, unknown>) => Promise<unknown> };
        }).__TAURI_INTERNALS__;
        return internals?.invoke?.("open_surface", { surface: target });
      },
      label,
    );
  } else {
    await invokeForTest(browser, "open_surface", { surface: label });
  }
  await browser.tauri?.switchWindow?.(`window-${surface}`);
  return waitForSurface(browser, surface);
}

export async function createIsolatedFixture(options?: Parameters<typeof createAppFixture>[0]): Promise<AppFixture> {
  return createAppFixture(options);
}

export async function assertBinaryExists(binary: string): Promise<void> {
  try {
    await access(binary);
  } catch {
    throw new Error(`E2E binary does not exist: ${binary}`);
  }
}
