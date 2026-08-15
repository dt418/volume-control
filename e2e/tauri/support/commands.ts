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
  refresh?: () => Promise<void>;
  tauri?: {
    execute: (script: unknown, ...args: unknown[]) => Promise<unknown>;
    switchWindow?: (label: string) => Promise<void>;
    listWindows?: () => Promise<string[]>;
    mock?: (command: string) => Promise<{ mockRejectedValue: (error: unknown) => Promise<unknown> }>;
    restoreAllMocks?: (commandPrefix?: string) => Promise<unknown>;
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
