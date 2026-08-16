import { browser, $, expect } from "@wdio/globals";
import { saveE2eArtifacts } from "../support/artifacts.ts";
import { assertNoRuntimeErrors, invokeForTest, type E2eBrowser } from "../support/commands.ts";

const app = browser as unknown as E2eBrowser;

// macOS/Linux-only surface: Windows routes overlay notifications to its
// native Win32 HUD and never opens the webview overlay (WindowManager
// rejects SurfaceId::Overlay on Windows), so this spec runs only on
// macOS/Linux CI where the webview HUD path is live.
describe("Overlay HUD surface", () => {
  afterEach(async function (this: { currentTest?: { state?: string; title?: string } }) {
    if (this.currentTest?.state === "failed") {
      await saveE2eArtifacts(browser as unknown as Record<string, unknown>, process.env.TAURI_E2E_OUTPUT ?? "output/tauri-e2e", `overlay-${this.currentTest.title ?? "unknown"}`);
    }
  });

  it("renders the HUD after a volume action and auto-hides", async () => {
    // The debug marker started this run with window-overlay open.
    await (browser as unknown as { tauri?: { switchWindow?: (label: string) => Promise<void> } }).tauri?.switchWindow?.("window-overlay");
    // Trigger a confirmed-state publish so the host emits state://overlay.
    await invokeForTest(app, "adjust_volume", { deltaPercent: 5 });
    const card = $('[data-surface="overlay"] [data-testid="overlay-card"]');
    await card.waitForDisplayed({ timeout: 8_000 });
    const rail = $('[data-surface="overlay"] [data-testid="signal-rail"]');
    await rail.waitForDisplayed({ timeout: 8_000 });
    const percent = $('[data-surface="overlay"] span.tabular-nums');
    expect((await percent.getText()).trim()).toMatch(/^\d+%$/);
    // Auto-hide: the host timer (and the frontend self-hide fallback) close
    // the window after the default overlay duration (1800 ms).
    await card.waitForExist({ timeout: 10_000, reverse: true });
    await assertNoRuntimeErrors(app);
  });
});
