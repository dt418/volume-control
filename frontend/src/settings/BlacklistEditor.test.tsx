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
function Harness({
  initial,
  saved,
}: {
  initial: SettingsConfig;
  saved?: string[];
}) {
  const [draft, setDraft] = useState(initial);
  return <BlacklistEditor draft={draft} setDraft={setDraft} saved={saved} />;
}

function renderEditor(overrides?: Partial<SettingsConfig>, saved?: string[]) {
  render(<Harness initial={{ ...baseDraft, ...overrides }} saved={saved} />);
}

describe("BlacklistEditor", () => {
  it("shows the legacy empty-state copy when the list is empty", () => {
    renderEditor();
    expect(screen.getByText("No blocked applications")).toBeInTheDocument();
    expect(
      screen.getByText("Volume Control will respond to shortcuts everywhere."),
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
    fireEvent.click(screen.getAllByRole("button", { name: /remove/i })[0]);
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

  it("Restore defaults replaces the draft with the backend presets", async () => {
    vi.mocked(ipc.invoke).mockResolvedValue(["Code.exe"]);
    renderEditor({ blacklist: ["custom.exe"] });
    fireEvent.click(screen.getByRole("button", { name: "Restore defaults" }));
    await vi.waitFor(() => {
      expect(screen.getByText("code.exe")).toBeInTheDocument();
      expect(screen.queryByText("custom.exe")).not.toBeInTheDocument();
    });
  });

  it("Reload saved discards draft edits", () => {
    renderEditor({ blacklist: ["draft.exe"] }, ["saved.exe"]);
    fireEvent.click(screen.getByRole("button", { name: "Reload saved" }));
    expect(screen.getByText("saved.exe")).toBeInTheDocument();
    expect(screen.queryByText("draft.exe")).not.toBeInTheDocument();
  });

  it("adds an app selected from the running-app picker", async () => {
    const code = {
      process: "code.exe",
      title: "Visual Studio Code",
      display_name: "Visual Studio Code",
      icon: "data:image/png;base64,AA==",
    };
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "running_apps" || cmd === "resolve_apps") return [code];
      return [];
    });
    renderEditor();
    fireEvent.click(screen.getByRole("button", { name: "Choose running apps" }));
    fireEvent.click(
      await screen.findByRole("checkbox", { name: /block visual studio code/i }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.getByText("Visual Studio Code")).toBeInTheDocument();
    expect(screen.queryByText("code.exe")).not.toBeInTheDocument();
    expect(
      document.querySelector('img[src="data:image/png;base64,AA=="]'),
    ).not.toBeNull();
  });

  it("keeps exact process names when the process style is selected", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "resolve_apps") {
        return [
          {
            process: "code.exe",
            title: "Visual Studio Code",
            display_name: "Visual Studio Code",
          },
        ];
      }
      return [];
    });
    renderEditor({ blacklist: ["code.exe"], app_label_style: "process" });
    expect(screen.getByText("code.exe")).toBeInTheDocument();
    expect(screen.queryByText("Visual Studio Code")).not.toBeInTheDocument();
  });
});
