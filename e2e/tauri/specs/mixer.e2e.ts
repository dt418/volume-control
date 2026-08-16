import { browser, $, expect } from "@wdio/globals";
import { after, before } from "mocha";
import { saveE2eArtifacts } from "../support/artifacts.ts";
import {
  invokeForTest,
  openSurface,
  waitForSurface,
  type E2eBrowser,
} from "../support/commands.ts";
import { selectors } from "../support/selectors.ts";

const app = browser as unknown as E2eBrowser;

// Real-audio runs mutate the actual OS endpoint; capture the initial state
// and restore it after the suite so the device is never left at 50%/muted.
const initialAudio = { volume_pct: 0, muted: false };

describe("Mixer surface", () => {
  before(async () => {
    const bootstrap = (await invokeForTest(app, "get_bootstrap", {})) as {
      volume_pct?: number;
      muted?: boolean;
    };
    initialAudio.volume_pct = bootstrap.volume_pct ?? 50;
    initialAudio.muted = bootstrap.muted ?? false;
  });

  beforeEach(async () => {
    await openSurface(app, "mixer");
  });

  after(async () => {
    // Restore the device state captured before the suite.
    await invokeForTest(app, "set_volume", { percent: initialAudio.volume_pct });
    const current = (await invokeForTest(app, "get_bootstrap", {})) as {
      muted?: boolean;
    };
    if ((current.muted ?? false) !== initialAudio.muted) {
      await invokeForTest(app, "toggle_mute", {});
    }
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
    // Native session support is Windows-only, while the empty filtered state
    // is also valid on Windows when no sessions are present. Keep this check
    // platform-agnostic so Linux/macOS validate their intentional fallback.
    expect(["No matching apps", "No audio sessions", "Per-app audio isn't available on this platform."]).toContain(
      await emptyMessage.getText(),
    );
    await search.clearValue();
    await $(selectors.mixer.mute).click();
    await $(selectors.mixer.reset).click();
    await expect($(selectors.mixer.systemVolume)).toBeDisplayed();
  });

  it("round-trips system mute state through the backend event", async () => {
    const mute = $(selectors.mixer.mute);
    const unmute = $('[aria-label="Unmute system output"]');
    const volume = $(selectors.mixer.systemVolume);

    // Establish a deterministic non-zero restore point for the backend
    // scalar-volume assertion below.
    await $(selectors.mixer.reset).click();
    await expect(volume).toHaveAttribute("aria-valuenow", "50");

    // Normalize the starting state in case a previous diagnostic run left the
    // default endpoint muted.
    if (await unmute.isExisting()) {
      await unmute.click();
      await expect(mute).toBeDisplayed();
    }

    await mute.click();
    await expect(unmute).toBeDisplayed();
    await expect($("[data-testid=system-output-value]")).toHaveText("Muted");
    await expect(volume).toHaveAttribute("aria-valuenow", "0");

    await unmute.click();
    await expect(mute).toBeDisplayed();
    await expect($("[data-testid=system-output-value]")).not.toHaveText("Muted");
    await expect(volume).toHaveAttribute("aria-valuenow", "50");
  });
});
