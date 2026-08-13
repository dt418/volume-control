import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import * as ipc from "../lib/ipc";
import { MixerSurface } from "./MixerSurface";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(),
  listen: vi.fn(async () => () => {}),
}));

const bootstrap = {
  volume_pct: 55,
  muted: false,
  hotkey_status: [],
  appearance: {},
  sessions: [
    { id: "a", name: "Spotify", pct: 40, muted: false, active: true },
    { id: "b", name: "Game", pct: 90, muted: false, active: false },
    { id: "c", name: "MutedApp", pct: 10, muted: true, active: false },
  ],
  sessions_supported: true,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_bootstrap") return bootstrap;
    return {};
  });
});

async function renderSurface() {
  render(<MixerSurface />);
  await screen.findAllByTestId("session-row");
}

describe("MixerSurface", () => {
  it("renders sessions with active apps sorted first", async () => {
    await renderSurface();
    const rows = screen.getAllByTestId("session-row");
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("Spotify"); // active first
    expect(rows[1]).toHaveTextContent("Game"); // then by pct desc
    expect(rows[2]).toHaveTextContent("MutedApp");
  });

  it("filters by search term", async () => {
    await renderSurface();
    fireEvent.change(screen.getByPlaceholderText(/search/i), {
      target: { value: "spot" },
    });
    const rows = screen.getAllByTestId("session-row");
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveTextContent("Spotify");
  });

  it("toggles mute for a session", async () => {
    await renderSurface();
    const mute = screen.getAllByRole("button", { name: /mute/i })[2];
    fireEvent.click(mute);
    expect(ipc.invoke).toHaveBeenCalledWith("mute_session", { id: "c" });
  });

  it("closes the window on Escape", async () => {
    await renderSurface();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-mixer",
    });
  });

  it("removes a stale session row and shows a notice when the slider write fails", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") return bootstrap;
      if (cmd === "set_session_volume") throw new Error("session not found");
      return {};
    });
    await renderSurface();
    const slider = screen.getAllByRole("slider")[0]; // Spotify row
    fireEvent.keyDown(slider, { key: "ArrowRight" }); // Radix slider step
    await waitFor(() => {
      expect(screen.getAllByTestId("session-row")).toHaveLength(2);
    });
    expect(screen.getByRole("status")).toHaveTextContent(/session removed/);
  });

  it("shows an empty state when no audio sessions exist", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return { ...bootstrap, sessions: [], sessions_supported: true };
      }
      return {};
    });
    render(<MixerSurface />);
    expect(await screen.findByText("No audio sessions")).toBeInTheDocument();
  });

  it("shows the Windows-only notice when sessions are unsupported", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return { ...bootstrap, sessions: [], sessions_supported: false };
      }
      return {};
    });
    render(<MixerSurface />);
    expect(
      await screen.findByText("Per-app mixing is Windows-only"),
    ).toBeInTheDocument();
  });
});
