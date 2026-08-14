import { browser, $, expect } from "@wdio/globals";
import { saveE2eArtifacts, recordTiming } from "../support/artifacts.ts";
import { openSurface, type E2eBrowser } from "../support/commands.ts";
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
    await $(selectors.help.close).click();
  });
});
