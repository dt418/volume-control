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
    modifier: "CtrlAlt",
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
    () => expect(screen.getByRole("button", { name: "Ctrl + Alt" })).toBeInTheDocument(),
    { timeout: 3000 },
  );
}

describe("SettingsSurface", () => {
  it("renders the modifier picker with CapsLock disabled and its fallback note", async () => {
    await renderSurface();
    for (const label of ["Ctrl + Alt", "Alt", "Ctrl"]) {
      // Exact string match: `new RegExp(label)` would treat the literal "+"
      // in "Ctrl + Alt" as a quantifier and never match.
      expect(screen.getByRole("button", { name: label })).toBeEnabled();
    }
    const caps = screen.getByRole("button", { name: /caps lock/i });
    expect(caps).toBeDisabled();
    expect(screen.getByText(/falls back to Ctrl \+ Alt/i)).toBeInTheDocument();
  });

  it("calls set_modifier when an enabled modifier is picked", async () => {
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: /^Alt$/i }));
    expect(ipc.invoke).toHaveBeenCalledWith("set_modifier", { modifier: "Alt" });
  });

  it("shows a red conflict badge with the error text for a Conflicted action", async () => {
    await renderSurface();
    const badge = screen.getByText(/hotkey already registered by another application/i);
    expect(badge).toBeInTheDocument();
    expect(badge.closest(".destructive")).not.toBeNull();
  });

  it("commits step-size edits through update_settings", async () => {
    await renderSurface();
    const small = screen.getByLabelText(/^Volume step$/i) as HTMLInputElement;
    fireEvent.change(small, { target: { value: "5" } });
    fireEvent.blur(small);
    expect(ipc.invoke).toHaveBeenCalledWith("update_settings", {
      patch: { volume_step: 5 },
    });
  });

  it("commits appearance changes through update_settings", async () => {
    await renderSurface();
    fireEvent.change(screen.getByLabelText(/theme/i), {
      target: { value: "Dark" },
    });
    expect(ipc.invoke).toHaveBeenCalledWith("update_settings", {
      patch: { theme: "Dark" },
    });
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
    expect(screen.queryByText(/already registered.*VolumeUp/i)).not.toBeInTheDocument();
    handler?.([
      { action: "VolumeUp", status: { Conflicted: { error_code: 1409, message: "conflict" } } },
    ]);
    expect(await screen.findByText(/conflict/i)).toBeInTheDocument();
  });
});
