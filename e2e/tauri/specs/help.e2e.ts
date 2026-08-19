import { browser, $, expect } from "@wdio/globals";
import { saveE2eArtifacts, recordTiming } from "../support/artifacts.ts";
import { openSurface, waitForSurface, waitForWindow, type E2eBrowser } from "../support/commands.ts";
import { selectors } from "../support/selectors.ts";

const app = browser as unknown as E2eBrowser;

describe("Help surface", () => {
  beforeEach(async () => {
    await openSurface(app, "help");
  });

  afterEach(async function (this: { currentTest?: { state?: string; title?: string } }) {
    if (this.currentTest?.state === "failed") {
      await saveE2eArtifacts(browser as unknown as Record<string, unknown>, process.env.TAURI_E2E_OUTPUT ?? "output/tauri-e2e", `help-${this.currentTest.title ?? "unknown"}`);
    }
  });

  it("renders shortcut cards, status badges, and footer actions", async () => {
    const started = performance.now();
    await expect($(selectors.surfaceTitle("help"))).toHaveText("VolumeControl");
    await expect($(selectors.header("help"))).toBeDisplayed();
    await expect($(selectors.content("help"))).toBeDisplayed();
    await expect($(selectors.footer("help"))).toBeDisplayed();
    await expect($("[data-testid=hotkey-status-badge]")).toBeDisplayed();
    await expect($(selectors.help.settings)).toBeDisplayed();
    recordTiming("help-to-ready", performance.now() - started);
  });

  it("filters shortcuts and keeps the close control accessible", async () => {
    await $(selectors.help.search).setValue("volume");
    await expect($("[data-testid=help-card]")).toBeDisplayed();
    await expect($(selectors.help.close)).toBeDisplayed();
    // Clicking Close destroys the current native WebView. On macOS/WebKit
    // the embedded driver can close its own transport before it receives the
    // command response, yielding a false ECONNREFUSED. The close IPC path is
    // covered by HelpSurface tests; this E2E case verifies the control remains
    // rendered and accessible after filtering.
  });

  it("opens the Settings surface from the footer without crashing the host", async () => {
    // Regression: clicking Settings while Help is open used to tear down the
    // app (Tauri exits when the last webview closes; the second surface also
    // exercised the open-surface path from a live WebView). The host must
    // stay alive and the Settings surface must become visible.
    await expect($(selectors.help.settings)).toBeDisplayed();
    await $(selectors.help.settings).click();
    await waitForWindow(app, "window-settings");
    await app.tauri?.switchWindow?.("window-settings");
    await waitForSurface(app, "settings");
    await expect($(selectors.settings.save)).toBeDisplayed();
  });
});
