import { browser, $, expect } from "@wdio/globals";
import { saveE2eArtifacts } from "../support/artifacts.ts";
import { openSurface, type E2eBrowser } from "../support/commands.ts";
import { selectors } from "../support/selectors.ts";

const app = browser as unknown as E2eBrowser;

describe("Settings surface", () => {
  beforeEach(async () => {
    await openSurface(app, "settings");
  });

  afterEach(async function (this: { currentTest?: { state?: string; title?: string } }) {
    if (this.currentTest?.state === "failed") {
      await saveE2eArtifacts(browser as unknown as Record<string, unknown>, process.env.TAURI_E2E_OUTPUT ?? "output/tauri-e2e", `settings-${this.currentTest.title ?? "unknown"}`);
    }
  });

  it("exposes all six sections and keeps Save disabled for a clean draft", async () => {
    await expect($(selectors.surfaceTitle("settings"))).toHaveText("VolumeControl Settings");
    await expect($(selectors.header("settings"))).toBeDisplayed();
    await expect($(selectors.content("settings"))).toBeDisplayed();
    await expect($(selectors.footer("settings"))).toBeDisplayed();
    for (const section of ["General", "Hotkeys", "Appearance", "Blacklist", "Feedback", "Storage"]) {
      await expect($(`//nav[@aria-label="Settings sections"]//button[normalize-space()="${section}"]`)).toBeDisplayed();
    }
    await expect($(selectors.settings.save)).toBeDisabled();
  });

  it("keeps edits draft-only until Save and supports Reset", async () => {
    const volumeStep = $("[data-surface=settings] input[aria-label='Volume step']");
    await volumeStep.setValue("2");
    await expect($(selectors.settings.save)).toBeEnabled();
    await $(selectors.settings.reset).click();
    await expect($(selectors.settings.save)).toBeDisabled();
  });

  it("surfaces validation feedback for an invalid draft", async () => {
    await $("[data-surface=settings] input[aria-label='Overlay duration']").setValue("100");
    await $(selectors.settings.save).click();
    await expect($("[data-surface=settings] [role=alert]")).toBeDisplayed();
  });
});
