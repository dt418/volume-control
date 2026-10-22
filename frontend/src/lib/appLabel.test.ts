import { describe, expect, it } from "vitest";

import { appLabel, asAppLabelStyle } from "./appLabel";

describe("appLabel", () => {
  it("defaults unknown styles to friendly names", () => {
    expect(asAppLabelStyle(undefined)).toBe("friendly");
    expect(asAppLabelStyle("bogus")).toBe("friendly");
    expect(appLabel("friendly", "Google Chrome", "chrome.exe")).toBe("Google Chrome");
  });

  it("supports friendly plus raw process names", () => {
    expect(appLabel("friendly_and_process", "Google Chrome", "chrome.exe")).toBe(
      "Google Chrome (chrome.exe)",
    );
  });

  it("keeps exact process names in process mode and falls back without a display name", () => {
    expect(appLabel("process", "Google Chrome", "chrome.exe")).toBe("chrome.exe");
    expect(appLabel("friendly", undefined, "chrome.exe")).toBe("chrome.exe");
  });
});
