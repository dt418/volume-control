import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import * as ipc from "../lib/ipc";
import { HelpSurface } from "./HelpSurface";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(async () => ({})),
  listen: vi.fn(async () => () => {}),
}));

const bootstrap = {
  config: { modifier: "CtrlAlt" },
  appearance: { theme_resolved: "dark", material: "Auto", motion: "Full", accent: "System" },
  hotkey_status: [
    { action: "VolumeUp", status: "Registered" },
    { action: "VolumeDown", status: "Registered" },
    { action: "VolumeUpLarge", status: "Registered" },
    { action: "VolumeDownLarge", status: "Registered" },
    { action: "ToggleMute", status: "Registered" },
    { action: "OpenMenu", status: "Registered" },
    { action: "Reset50", status: "Registered" },
    { action: "OpenMixer", status: "Registered" },
  ],
};

beforeEach(() => {
  vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_bootstrap") return bootstrap;
    return {};
  });
});

async function renderSurface() {
  render(<HelpSurface />);
  await waitFor(() => expect(screen.getByText("VolumeControl")).toBeInTheDocument(), {
    timeout: 3000,
  });
}

describe("HelpSurface", () => {
  it("keeps header, shortcut content, and footer inside a bounded shell", async () => {
    await renderSurface();
    expect(screen.getByRole("main")).toHaveAttribute("data-surface", "help");
    expect(screen.getByTestId("surface-header")).toBeInTheDocument();
    expect(screen.getByTestId("surface-content")).toBeInTheDocument();
    expect(screen.getByTestId("surface-footer")).toBeInTheDocument();
  });

  it("renders the legacy header band with the close affordance", async () => {
    await renderSurface();
    expect(screen.getByText("VolumeControl")).toBeInTheDocument();
    expect(screen.getByText("Keyboard shortcuts")).toBeInTheDocument();
    // Header × is aria-label "Close" (pointer-only, like legacy); the footer
    // also has a visible Close button.
    fireEvent.click(screen.getAllByRole("button", { name: "Close" })[0]);
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-help",
    });
  });

  it("renders base + extended shortcut cards grouped by section", async () => {
    await renderSurface();
    expect(screen.getByText("Volume")).toBeInTheDocument();
    expect(screen.getByText("Commands")).toBeInTheDocument();
    expect(screen.getByText("Extended")).toBeInTheDocument();

    const cards = screen.getAllByTestId("help-card");
    // 2 volume base + 3 command base + 3 extended
    expect(cards).toHaveLength(8);

    expect(screen.getByText("Volume Up")).toBeInTheDocument();
    expect(screen.getByText("Volume Down")).toBeInTheDocument();
    expect(screen.getByText("Volume Up (large)")).toBeInTheDocument();
    expect(screen.getByText("Volume Down (large)")).toBeInTheDocument();
    expect(screen.getByText("Toggle Mute")).toBeInTheDocument();
    expect(screen.getByText("Open Menu")).toBeInTheDocument();
    expect(screen.getByText("Reset to 50%")).toBeInTheDocument();
    expect(screen.getByText("Open Mixer")).toBeInTheDocument();
  });

  it("shows a Ready badge per row when every action registered", async () => {
    await renderSurface();
    const badges = screen.getAllByTestId("hotkey-status-badge");
    expect(badges).toHaveLength(8);
    for (const badge of badges) {
      expect(badge).toHaveTextContent("Ready");
    }
  });

  it("renders an In use badge for a conflicted action", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return {
          ...bootstrap,
          hotkey_status: bootstrap.hotkey_status.map((r) =>
            r.action === "ToggleMute"
              ? { action: "ToggleMute", status: { Conflicted: { error_code: 1409, message: "taken" } } }
              : r,
          ),
        };
      }
      return {};
    });
    await renderSurface();
    const inUse = screen.getAllByTestId("hotkey-status-badge").filter((b) =>
      b.textContent?.includes("In use"),
    );
    expect(inUse).toHaveLength(1);
  });

  it("shows the conflict callout when a base action is in use and the CTA opens Settings", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return {
          ...bootstrap,
          hotkey_status: bootstrap.hotkey_status.map((r) =>
            r.action === "ToggleMute"
              ? { action: "ToggleMute", status: { Conflicted: { error_code: 1409, message: "taken" } } }
              : r,
          ),
        };
      }
      return {};
    });
    await renderSurface();
    const callout = screen.getByTestId("conflict-callout");
    expect(callout).toBeInTheDocument();
    expect(screen.getByText("Shortcut conflict")).toBeInTheDocument();
    expect(screen.getByText(/is used by another app/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Change the modifier in Settings/i }));
    expect(ipc.invoke).toHaveBeenCalledWith("open_surface", {
      surface: "window-settings",
    });
  });

  it("hides the conflict callout when nothing is conflicted", async () => {
    await renderSurface();
    expect(screen.queryByTestId("conflict-callout")).not.toBeInTheDocument();
  });

  it("filters cards by a search box", async () => {
    await renderSurface();
    fireEvent.change(screen.getByPlaceholderText(/search/i), {
      target: { value: "mute" },
    });
    expect(screen.getAllByTestId("help-card")).toHaveLength(1);
    expect(screen.getByText("Toggle Mute")).toBeInTheDocument();
    expect(screen.queryByText("Volume Up")).not.toBeInTheDocument();
  });

  it("shows a Kbd badge with the current modifier's combo on each card", async () => {
    await renderSurface();
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

  it("footer Edit config opens the config file and Settings opens the settings surface", async () => {
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Edit config" }));
    expect(ipc.invoke).toHaveBeenCalledWith("open_config_location");
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(ipc.invoke).toHaveBeenCalledWith("open_surface", {
      surface: "window-settings",
    });
  });

  it("closes via Escape", async () => {
    await renderSurface();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-help",
    });
  });

  it("updates badges live from state://hotkeys events", async () => {
    let handler: ((payload: unknown) => void) | undefined;
    vi.mocked(ipc.listen).mockImplementation(
      async (_event: string, cb: (payload: unknown) => void) => {
        handler = cb;
        return () => {};
      },
    );
    await renderSurface();
    expect(screen.queryByText("In use")).not.toBeInTheDocument();
    handler?.(
      bootstrap.hotkey_status.map((r) =>
        r.action === "Reset50"
          ? { action: "Reset50", status: { Conflicted: { error_code: 1409, message: "taken" } } }
          : r,
      ),
    );
    expect(await screen.findByText("In use")).toBeInTheDocument();
  });
});
