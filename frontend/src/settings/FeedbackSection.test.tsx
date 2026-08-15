import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { FeedbackSection } from "./FeedbackSection";
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

function renderSection() {
  let current = draft;
  const setDraft = (update: (d: SettingsConfig) => SettingsConfig) => {
    current = update(current);
  };
  render(<FeedbackSection draft={current} setDraft={setDraft} errors={{}} />);
  return { getDraft: () => current };
}

describe("FeedbackSection", () => {
  it("renders the beep switch and the four legacy-labelled inputs", () => {
    renderSection();
    expect(screen.getByLabelText("Enable beep feedback")).toBeChecked();
    for (const label of [
      "Blocked beep frequency",
      "Blocked beep duration",
      "Limit beep frequency",
      "Limit beep duration",
    ]) {
      expect(screen.getByLabelText(label)).toBeInTheDocument();
    }
    // Legacy helpers.
    expect(screen.getByText("Beep when a shortcut is blocked.")).toBeInTheDocument();
    expect(
      screen.getByText("Beep when volume is already at the limit."),
    ).toBeInTheDocument();
  });

  it("edits the draft from the inputs", () => {
    const { getDraft } = renderSection();
    fireEvent.click(screen.getByLabelText("Enable beep feedback"));
    expect(getDraft().beep.enabled).toBe(false);
    fireEvent.change(screen.getByLabelText(/^Blocked beep frequency$/i), {
      target: { value: "500" },
    });
    expect(getDraft().beep.blocked_freq).toBe(500);
  });

  it("shows an inline error for a backend-rejected field", () => {
    render(
      <FeedbackSection
        draft={draft}
        setDraft={() => {}}
        errors={{ "beep.limit_duration_ms": "beep.limit_duration_ms: must be between 10 and 2000" }}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("must be between 10 and 2000");
  });
});
