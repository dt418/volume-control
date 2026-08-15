import { browser, $, expect } from "@wdio/globals";
import { saveE2eArtifacts } from "../support/artifacts.ts";
import { openSurface, waitForSurface, type E2eBrowser } from "../support/commands.ts";
import { selectors } from "../support/selectors.ts";

const app = browser as unknown as E2eBrowser;

describe("surface recovery", () => {
  beforeEach(async () => {
    await openSurface(app, "mixer");
  });

  afterEach(async function (this: { currentTest?: { state?: string; title?: string } }) {
    if (this.currentTest?.state === "failed") {
      await saveE2eArtifacts(browser as unknown as Record<string, unknown>, process.env.TAURI_E2E_OUTPUT ?? "output/tauri-e2e", `recovery-${this.currentTest.title ?? "unknown"}`);
    }
  });

  it("shows a readable alert and keeps the shell mounted after bootstrap failure", async () => {
    await waitForSurface(app, "mixer");
    await expect($(selectors.surfaceRoot("mixer"))).toBeDisplayed();
    await expect($('[data-surface="mixer"] [role="alert"]')).toHaveText(/Mixer connection needs attention.*E2E bootstrap failure/);
  });
});
