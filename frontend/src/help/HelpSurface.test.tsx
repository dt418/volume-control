import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { HelpSurface } from "./HelpSurface";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "get_bootstrap") {
      return {
        config: { modifier: "CtrlAlt" },
      };
    }
    return {};
  }),
  listen: vi.fn(async () => () => {}),
}));

describe("HelpSurface", () => {
  it("renders all shortcut cards grouped by Volume / Commands", async () => {
    render(<HelpSurface />);
    expect(await screen.findByText("Volume")).toBeInTheDocument();
    expect(screen.getByText("Commands")).toBeInTheDocument();

    const cards = screen.getAllByTestId("help-card");
    // 4 volume + 4 command shortcuts
    expect(cards).toHaveLength(8);

    // Labels from the fixed shortcut set
    expect(screen.getByText("Volume Up")).toBeInTheDocument();
    expect(screen.getByText("Volume Down")).toBeInTheDocument();
    expect(screen.getByText("Volume Up (large)")).toBeInTheDocument();
    expect(screen.getByText("Volume Down (large)")).toBeInTheDocument();
    expect(screen.getByText("Toggle Mute")).toBeInTheDocument();
    expect(screen.getByText("Open Menu")).toBeInTheDocument();
    expect(screen.getByText("Reset to 50%")).toBeInTheDocument();
    expect(screen.getByText("Open Mixer")).toBeInTheDocument();
  });

  it("filters cards by a search box", async () => {
    render(<HelpSurface />);
    await screen.findByText("Volume");

    fireEvent.change(screen.getByPlaceholderText(/search/i), {
      target: { value: "mute" },
    });

    expect(screen.getAllByTestId("help-card")).toHaveLength(1);
    expect(screen.getByText("Toggle Mute")).toBeInTheDocument();
    expect(screen.queryByText("Volume Up")).not.toBeInTheDocument();
  });

  it("shows a Kbd badge with the current modifier's combo on each card", async () => {
    render(<HelpSurface />);
    await screen.findByText("Volume");

    const combos = screen.getAllByTestId("help-combo");
    expect(combos.map((c) => c.textContent)).toEqual(
      expect.arrayContaining([
        "Ctrl+Alt+↑",
        "Ctrl+Alt+↓",
        "Ctrl+Alt+Shift+↑",
        "Ctrl+Alt+Shift+↓",
        "Ctrl+Alt+M",
        "Ctrl+Alt+Shift+M",
        "Ctrl+Alt+R",
        "Ctrl+Alt+V",
      ]),
    );
  });
});
