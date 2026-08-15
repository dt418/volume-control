import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import * as ipc from "../lib/ipc";
import { BlacklistEditor } from "./BlacklistEditor";
import type { SettingsConfig } from "./settingsTypes";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(async () => []),
  listen: vi.fn(async () => () => {}),
}));

const baseDraft: SettingsConfig = {
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

/** Stateful wrapper so the draft prop re-renders on setDraft (mirrors how
 *  SettingsSurface drives the editor). */
function Harness({ initial }: { initial: SettingsConfig }) {
  const [draft, setDraft] = useState(initial);
  return <BlacklistEditor draft={draft} setDraft={setDraft} />;
}

function renderEditor(overrides?: Partial<SettingsConfig>) {
  render(<Harness initial={{ ...baseDraft, ...overrides }} />);
}

describe("BlacklistEditor", () => {
  it("shows the legacy empty-state copy when the list is empty", () => {
    renderEditor();
    expect(screen.getByText("No blocked applications")).toBeInTheDocument();
    expect(
      screen.getByText("VolumeControl will respond to shortcuts everywhere."),
    ).toBeInTheDocument();
  });

  it("adds a normalized entry and rejects duplicates", () => {
    renderEditor();
    const input = screen.getByLabelText(/blocked application/i);
    fireEvent.change(input, { target: { value: "  Code.exe " } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByText("code.exe")).toBeInTheDocument();
    // Duplicate add (already present, case-insensitive) is a no-op.
    fireEvent.change(input, { target: { value: "Code.exe" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getAllByText("code.exe")).toHaveLength(1);
  });

  it("removes and clears entries", () => {
    renderEditor({ blacklist: ["code.exe", "chrome.exe"] });
    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    expect(screen.getByText("chrome.exe")).toBeInTheDocument();
    expect(screen.queryByText("code.exe")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(screen.getByText("No blocked applications")).toBeInTheDocument();
  });

  it("Apply Recommended merges the backend presets without duplicates", async () => {
    vi.mocked(ipc.invoke).mockResolvedValue(["Code.exe", "idea64.exe"]);
    renderEditor({ blacklist: ["code.exe"] });
    fireEvent.click(screen.getByRole("button", { name: "Apply Recommended" }));
    await vi.waitFor(() => {
      expect(screen.getByText("code.exe")).toBeInTheDocument();
      expect(screen.getByText("idea64.exe")).toBeInTheDocument();
    });
    expect(ipc.invoke).toHaveBeenCalledWith("recommended_blacklist");
  });
});
