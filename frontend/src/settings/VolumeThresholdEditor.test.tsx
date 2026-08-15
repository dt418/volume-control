import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { VolumeThresholdEditor } from "./VolumeThresholdEditor";
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

function renderEditor() {
  let current = draft;
  const setDraft = (update: (d: SettingsConfig) => SettingsConfig) => {
    current = update(current);
  };
  render(
    <VolumeThresholdEditor draft={current} setDraft={setDraft} errors={{}} />,
  );
  return { getDraft: () => current };
}

describe("VolumeThresholdEditor", () => {
  it("renders the three threshold inputs with draft values", () => {
    renderEditor();
    expect(screen.getByLabelText("Green up to")).toHaveValue(40);
    expect(screen.getByLabelText("Blue up to")).toHaveValue(75);
    expect(screen.getByLabelText("Orange up to")).toHaveValue(100);
  });

  it("edits the draft thresholds", () => {
    const { getDraft } = renderEditor();
    fireEvent.change(screen.getByLabelText("Green up to"), {
      target: { value: "30" },
    });
    expect(getDraft().color_thresholds.green_up_to).toBe(30);
  });

  it("shows an inline error from the backend field name", () => {
    render(
      <VolumeThresholdEditor
        draft={draft}
        setDraft={() => {}}
        errors={{
          "color_thresholds.green_up_to":
            "color_thresholds.green_up_to: must not exceed blue_up_to",
        }}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "must not exceed blue_up_to",
    );
    expect(screen.getByLabelText("Green up to")).toHaveAttribute(
      "aria-invalid",
      "true",
    );
  });
});
