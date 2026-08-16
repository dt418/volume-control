import { browser, $, expect } from "@wdio/globals";
import { saveE2eArtifacts } from "../support/artifacts.ts";
import { assertNoRuntimeErrors, invokeForTest, type E2eBrowser } from "../support/commands.ts";

const app = browser as unknown as E2eBrowser;

describe("Overlay HUD surface", () => {
  afterEach(async function (this: { currentTest?: { state?: string; title?: string } }) {
    if (this.currentTest?.state === "failed") {
      await saveE2eArtifacts(browser as unknown as Record<string, unknown>, process.env.TAURI_E2E_OUTPUT ?? "output/tauri-e2e", `overlay-${this.currentTest.title ?? "unknown"}`);
    }
  });

  it("renders the HUD after a volume action and auto-hides", async () => {
    // The debug marker started this run with window-overlay open.
    await (browser as unknown as { tauri?: { switchWindow?: (label: string) => Promise<void> } }).tauri?.switchWindow?.("window-overlay");
    // Trigger a confirmed-state publish so the host emits state://overlay
    // (macOS/Linux). Tauri commands rename snake_case arguments to camelCase.
    await invokeForTest(app, "adjust_volume", { deltaPercent: 5 });
    if (process.platform === "win32") {
      // Windows routes overlay notifications to the native HUD
      // (TauriSink::overlay -> NativeWin32), so the webview overlay never
      // receives state://overlay there; the macOS/Linux webview HUD path is
      // asserted below. This branch still proves the surface boots with the
      // capability wiring (core invoke + event listen allowed from
      // window-overlay) and that the WDIO debug bridge is live.
      const title = await (browser as unknown as { getTitle?: () => Promise<string> }).getTitle?.();
      expect(title).toContain("Overlay");
      const wdioBridge = await (browser as unknown as {
        execute: (script: () => unknown) => Promise<unknown>;
      }).execute(() => typeof (window as Window & { wdioTauri?: { execute?: unknown } }).wdioTauri?.execute);
      expect(wdioBridge).toBe("function");
    } else {
      const card = $('[data-surface="overlay"] [data-testid="overlay-card"]');
      await card.waitForDisplayed({ timeout: 8_000 });
      const rail = $('[data-surface="overlay"] [data-testid="signal-rail"]');
      await rail.waitForDisplayed({ timeout: 8_000 });
      const percent = $('[data-surface="overlay"] span.tabular-nums');
      expect((await percent.getText()).trim()).toMatch(/^\d+%$/);
      // Auto-hide: default overlay duration (1800 ms) destroys the window.
      await card.waitForExist({ timeout: 10_000, reverse: true });
    }
    await assertNoRuntimeErrors(app);
  });
});
