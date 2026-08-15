import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { AppearancePreview } from "./AppearancePreview";
import { bandForValue } from "../mixer/SignalRail";
import type { SettingsConfig } from "./settingsTypes";

const draft: SettingsConfig = {
  volume_step: 1,
  volume_step_large: 10,
  overlay_duration_ms: 1800,
  modifier: "CtrlAlt",
  blacklist: [],
  color_thresholds: { green_up_to: 40, blue_up_to: 75, orange_up_to: 100 },
  beep: {
    enabled: true,
    blocked_freq: 400,
    blocked_duration_ms: 80,
    limit_freq: 600,
    limit_duration_ms: 60,
  },
  appearance: { theme: "System", material: "Auto", motion: "Full", accent: "System" },
};

describe("AppearancePreview", () => {
  it("renders a mini Signal Rail driven by the draft thresholds", () => {
    const { container } = render(<AppearancePreview draft={draft} />);
    const rail = screen.getByTestId("signal-rail");
    expect(rail).toBeInTheDocument();
    // Sample value 65 with 40/75/100 falls in the blue band.
    const fill = container.querySelector(".rail-fill");
    expect(fill?.classList.contains("band-blue")).toBe(true);
    expect(bandForValue(65, false, draft.color_thresholds)).toBe("blue");
  });

  it("band classes track edited thresholds", () => {
    const lowBlue = { ...draft, color_thresholds: { green_up_to: 20, blue_up_to: 30, orange_up_to: 100 } };
    const { container } = render(<AppearancePreview draft={lowBlue} />);
    const fill = container.querySelector(".rail-fill");
    // Value 65 with 20/30/100 falls in the orange band.
    expect(fill?.classList.contains("band-orange")).toBe(true);
    expect(bandForValue(65, false, lowBlue.color_thresholds)).toBe("orange");
  });

  it("notes that the preview tracks the draft", () => {
    render(<AppearancePreview draft={draft} />);
    expect(screen.getByText(/preview tracks the draft/i)).toBeInTheDocument();
  });
});
