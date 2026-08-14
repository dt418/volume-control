import { browser, $, expect } from "@wdio/globals";
import { saveE2eArtifacts } from "../support/artifacts.ts";
import { openSurface, waitForSurface, type E2eBrowser } from "../support/commands.ts";
import { selectors } from "../support/selectors.ts";

const app = browser as unknown as E2eBrowser;

describe("Mixer surface", () => {
  beforeEach(async () => {
    await openSurface(app, "mixer");
  });

  afterEach(async function (this: { currentTest?: { state?: string; title?: string } }) {
    if (this.currentTest?.state === "failed") {
      await saveE2eArtifacts(browser as unknown as Record<string, unknown>, process.env.TAURI_E2E_OUTPUT ?? "output/tauri-e2e", `mixer-${this.currentTest.title ?? "unknown"}`);
    }
  });

  it("renders the bounded shell and accessible System Output controls", async () => {
    await waitForSurface(app, "mixer");
    await expect($(selectors.surfaceTitle("mixer"))).toHaveText("Volume Mixer");
    await expect($(selectors.header("mixer"))).toBeDisplayed();
    await expect($(selectors.content("mixer"))).toBeDisplayed();
    await expect($(selectors.footer("mixer"))).toBeDisplayed();
    await expect($("[data-testid=system-output-row]")).toBeDisplayed();
    await expect($(selectors.mixer.systemVolume)).toBeDisplayed();
    await expect($(selectors.mixer.mute)).toBeDisplayed();
    await expect($(selectors.mixer.reset)).toBeDisplayed();
  });

  it("filters sessions and exercises mute/reset without a fixed sleep", async () => {
    const search = $(selectors.mixer.search);
    await search.setValue("__no_matching_volumecontrol_session__");
    const emptyMessage = $('[data-surface="mixer"] p');
    await expect(emptyMessage).toBeDisplayed();
    expect(["No matching apps", "No audio sessions"]).toContain(await emptyMessage.getText());
    await search.clearValue();
    await $(selectors.mixer.mute).click();
    await $(selectors.mixer.reset).click();
    await expect($(selectors.mixer.systemVolume)).toBeDisplayed();
  });
});
