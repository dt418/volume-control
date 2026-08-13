import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import * as ipc from "../lib/ipc";
import { SettingsSurface } from "./SettingsSurface";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(async () => ({})),
  listen: vi.fn(async () => () => {}),
}));

const bootstrap = {
  volume_pct: 55,
  muted: false,
  sessions: [],
  sessions_supported: true,
  appearance: { theme_resolved: "dark", material: "Auto", motion: "Full", accent: "System" },
  hotkey_status: [
    { action: "VolumeUp", status: "Registered" },
    {
      action: "ToggleMute",
      status: {
        Conflicted: {
          error_code: 1409,
          message: "hotkey already registered by another application",
        },
      },
    },
  ],
  config: {
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
  },
};

beforeEach(() => {
  vi.mocked(ipc.invoke).mockResolvedValue({});
  vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_bootstrap") return bootstrap;
    return {};
  });
});

async function renderSurface() {
  render(<SettingsSurface />);
  await waitFor(
    () => expect(screen.getByRole("button", { name: "General" })).toBeInTheDocument(),
    { timeout: 3000 },
  );
}

describe("SettingsSurface", () => {
  it("renders the six legacy sections in the nav", async () => {
    await renderSurface();
    for (const section of [
      "General",
      "Hotkeys",
      "Appearance",
      "Blacklist",
      "Feedback",
      "Storage",
    ]) {
      expect(screen.getByRole("button", { name: section })).toBeInTheDocument();
    }
  });

  it("switches the active content pane when a nav section is picked", async () => {
    await renderSurface();
    expect(screen.getByLabelText(/^Volume step$/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Blacklist" }));
    expect(screen.getByText("No blocked applications")).toBeInTheDocument();
    expect(screen.queryByLabelText(/^Volume step$/i)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Storage" }));
    expect(screen.getByRole("button", { name: "Open config file" })).toBeInTheDocument();
  });

  it("keeps edits in the draft — nothing is committed until Save changes", async () => {
    await renderSurface();
    const small = screen.getByLabelText(/^Volume step$/i) as HTMLInputElement;
    fireEvent.change(small, { target: { value: "5" } });
    expect(ipc.invoke).not.toHaveBeenCalledWith("update_settings", expect.anything());
    expect(screen.getByRole("button", { name: "Save changes" })).toBeEnabled();
  });

  it("Save commits the whole draft as one update_settings patch and a modifier change via set_modifier", async () => {
    await renderSurface();
    const small = screen.getByLabelText(/^Volume step$/i) as HTMLInputElement;
    fireEvent.change(small, { target: { value: "5" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() =>
      expect(ipc.invoke).toHaveBeenCalledWith("update_settings", {
        patch: expect.objectContaining({
          volume_step: 5,
          overlay_duration_ms: 1800,
          beep: expect.objectContaining({ enabled: true }),
          color_thresholds: expect.objectContaining({ green_up_to: 40 }),
        }),
      }),
    );
    // No modifier change: set_modifier must NOT fire.
    expect(ipc.invoke).not.toHaveBeenCalledWith("set_modifier", expect.anything());
  });

  it("Save with a changed modifier fires set_modifier after update_settings", async () => {
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Hotkeys" }));
    fireEvent.click(screen.getByRole("button", { name: /^Alt$/i }));
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() =>
      expect(ipc.invoke).toHaveBeenCalledWith("set_modifier", { modifier: "Alt" }),
    );
  });

  it("shows Saved status and disables Save once the draft is committed", async () => {
    await renderSurface();
    const small = screen.getByLabelText(/^Volume step$/i) as HTMLInputElement;
    fireEvent.change(small, { target: { value: "5" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    expect(await screen.findByText("Saved")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Reset" })).toBeDisabled();
  });

  it("a rejected save retains edits, shows an inline field error and switches to the owning section", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") return bootstrap;
      if (cmd === "update_settings") {
        return Promise.reject("overlay_duration_ms: must be between 200 and 10000");
      }
      return {};
    });
    await renderSurface();
    const overlay = screen.getByLabelText(/^Overlay duration$/i) as HTMLInputElement;
    fireEvent.change(overlay, { target: { value: "50" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("must be between 200 and 10000");
    // The edit is retained (the input still shows 50).
    expect(screen.getByLabelText(/^Overlay duration$/i)).toHaveValue(50);
    // Still on the General pane (owns overlay_duration_ms).
    expect(screen.getByLabelText(/^Overlay duration$/i)).toBeInTheDocument();
  });

  it("a rejected beep save switches to the Feedback section and shows the inline error there", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") return bootstrap;
      if (cmd === "update_settings") {
        return Promise.reject("beep.blocked_freq: must be between 37 and 32767");
      }
      return {};
    });
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Feedback" }));
    const freq = screen.getByLabelText(/^Blocked beep frequency$/i) as HTMLInputElement;
    fireEvent.change(freq, { target: { value: "20" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("beep.blocked_freq");
    expect(screen.getByLabelText(/^Blocked beep frequency$/i)).toHaveValue(20);
  });

  it("Reset restores the committed config and discards edits", async () => {
    await renderSurface();
    const small = screen.getByLabelText(/^Volume step$/i) as HTMLInputElement;
    fireEvent.change(small, { target: { value: "5" } });
    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(screen.getByLabelText(/^Volume step$/i)).toHaveValue(1);
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("Cancel closes the settings surface", async () => {
    await renderSurface();
    const small = screen.getByLabelText(/^Volume step$/i) as HTMLInputElement;
    fireEvent.change(small, { target: { value: "5" } });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-settings",
    });
  });

  it("renders the modifier picker with CapsLock disabled and its fallback note", async () => {
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Hotkeys" }));
    for (const label of ["Ctrl + Alt", "Alt", "Ctrl"]) {
      expect(screen.getByRole("button", { name: label })).toBeEnabled();
    }
    const caps = screen.getByRole("button", { name: /caps lock/i });
    expect(caps).toBeDisabled();
    expect(screen.getByText(/falls back to Ctrl \+ Alt/i)).toBeInTheDocument();
  });

  it("shows a red conflict badge with the error text for a Conflicted action", async () => {
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Hotkeys" }));
    const badge = screen.getByText(/hotkey already registered by another application/i);
    expect(badge).toBeInTheDocument();
    expect(badge.closest(".destructive")).not.toBeNull();
  });

  it("updates conflict badges live from state://hotkeys events", async () => {
    let handler: ((payload: unknown) => void) | undefined;
    vi.mocked(ipc.listen).mockImplementation(
      async (_event: string, cb: (payload: unknown) => void) => {
        handler = cb;
        return () => {};
      },
    );
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Hotkeys" }));
    expect(screen.queryByText(/already registered.*VolumeUp/i)).not.toBeInTheDocument();
    handler?.([
      { action: "VolumeUp", status: { Conflicted: { error_code: 1409, message: "conflict" } } },
    ]);
    expect(await screen.findByText(/conflict/i)).toBeInTheDocument();
  });
});
