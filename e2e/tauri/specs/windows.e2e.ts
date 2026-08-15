import { browser, expect } from "@wdio/globals";
import { recordTiming, saveE2eArtifacts } from "../support/artifacts.ts";
import { listOwnedWindows, openSurface, waitForSurface, type E2eBrowser } from "../support/commands.ts";

const app = browser as unknown as E2eBrowser;

describe("owned Tauri windows", () => {
  beforeEach(async () => {
    await openSurface(app, "mixer");
  });

  afterEach(async function (this: { currentTest?: { state?: string; title?: string } }) {
    if (this.currentTest?.state === "failed") {
      await saveE2eArtifacts(browser as unknown as Record<string, unknown>, process.env.TAURI_E2E_OUTPUT ?? "output/tauri-e2e", `windows-${this.currentTest.title ?? "unknown"}`);
    }
  });

  it("enumerates only owned windows and records bootstrap-to-ready timing", async () => {
    const started = performance.now();
    await waitForSurface(app, "mixer");
    recordTiming("mixer-to-ready", performance.now() - started);
    const labels = await listOwnedWindows(app);
    expect(labels).toContain("window-mixer");
    expect(labels.every((label) => label.startsWith("window-"))).toBe(true);
    const title = await app.tauri?.execute?.(() => document.title);
    expect(title).toBe("Volume Mixer");
  });
});
