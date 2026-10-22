import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";

import * as ipc from "../lib/ipc";
import { MixerSurface } from "./MixerSurface";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(),
  listen: vi.fn(async (event: string, cb: (payload: unknown) => void) => {
    listeners[event] = cb;
    return () => {};
  }),
}));

// Captured `listen` handlers keyed by event name (reset in beforeEach).
const listeners: Record<string, (payload: unknown) => void> = {};

function fire(event: string, payload: unknown) {
  listeners[event]?.(payload);
}

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
  for (const key of Object.keys(listeners)) delete listeners[key];
  vi.mocked(ipc.listen).mockImplementation(
    async (event: string, handler: (payload: unknown) => void) => {
      listeners[event] = handler;
      return () => {};
    },
  );
  vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_bootstrap") return bootstrap;
    return {};
  });
});

afterEach(() => {
  vi.restoreAllMocks();
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
    const rows = screen.getAllByTestId("session-row");
    const mutedRow = rows.find((r) => r.textContent?.includes("MutedApp"));
    expect(mutedRow).toBeDefined();
    fireEvent.click(within(mutedRow!).getByRole("button", { name: /mute/i }));
    expect(ipc.invoke).toHaveBeenCalledWith("mute_session", { id: "c" });
  });

  it("labels System Sounds and mutes by its stable session id", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return {
          ...bootstrap,
          sessions: [
            {
              id: "system-instance",
              name: "@%SystemRoot%\System32\AudioSrv.Dll,-202",
              pct: 100,
              muted: false,
              active: false,
              system: true,
            },
          ],
        };
      }
      return {};
    });
    await renderSurface();
    expect(screen.getByText("System sounds")).toBeInTheDocument();
    expect(screen.getByText("Notifications & alerts")).toBeInTheDocument();
    const row = screen.getByTestId("session-row");
    fireEvent.click(within(row).getByRole("button", { name: /mute/i }));
    expect(ipc.invoke).toHaveBeenCalledWith("mute_session", {
      id: "system-instance",
    });
  });

  it("uses friendly session names and honors the process-only style", async () => {
    const session = {
      id: "chrome",
      name: "Google Chrome",
      process: "chrome.exe",
      display_name: "Google Chrome",
      pct: 50,
      muted: false,
      active: true,
    };
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return { ...bootstrap, config: { app_label_style: "friendly" }, sessions: [session] };
      }
      return {};
    });
    await renderSurface();
    expect(screen.getByTestId("session-row")).toHaveTextContent("Google Chrome");
    expect(screen.getByTestId("session-row")).not.toHaveTextContent("chrome.exe");

    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return {
          ...bootstrap,
          config: { app_label_style: "process" },
          sessions: [session],
        };
      }
      return {};
    });
    await renderSurface();
    expect(screen.getAllByTestId("session-row").at(-1)).toHaveTextContent("chrome.exe");
  });

  it("shows and dismisses the in-app update banner", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") return bootstrap;
      if (cmd === "check_for_updates") {
        return {
          current_version: "0.1.10",
          latest_version: "v0.1.11",
          is_update_available: true,
          release_url: "https://example.test/v0.1.11",
          message: "Update available: v0.1.11",
        };
      }
      return {};
    });
    await renderSurface();
    const banner = await screen.findByTestId("update-banner");
    expect(banner).toHaveTextContent("Update available: v0.1.11");
    fireEvent.click(
      within(banner).getByRole("button", { name: /dismiss update notification/i }),
    );
    expect(screen.queryByTestId("update-banner")).not.toBeInTheDocument();
  });

  it("closes the window on Escape", async () => {
    await renderSurface();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-mixer",
    });
  });

  it("closes the window from the visible close control", async () => {
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Close mixer" }));
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-mixer",
    });
  });

  it("opens Settings and Help from the header actions", async () => {
    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Help" }));
    expect(ipc.invoke).toHaveBeenCalledWith("open_surface", {
      surface: "window-settings",
    });
    expect(ipc.invoke).toHaveBeenCalledWith("open_surface", {
      surface: "window-help",
    });
  });

  it("closes the window when the Rust side emits close_mixer_request while unfocused", async () => {
    await renderSurface();
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    expect(listeners["close_mixer_request"]).toBeDefined();
    listeners["close_mixer_request"](null);
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-mixer",
    });
  });

  it("ignores a stale close request after the mixer regains focus", async () => {
    await renderSurface();
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    listeners["close_mixer_request"](null);
    expect(ipc.invoke).not.toHaveBeenCalledWith("close_surface", {
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
    const rows = screen.getAllByTestId("session-row");
    const slider = within(rows[0]).getByRole("slider"); // Spotify row (system slider is a separate control)
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

  it("shows the unsupported-platform notice when sessions are unsupported", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return { ...bootstrap, sessions: [], sessions_supported: false };
      }
      return {};
    });
    render(<MixerSurface />);
    expect(
      await screen.findByText("Per-app audio isn't available on this platform."),
    ).toBeInTheDocument();
  });

  it("offers a retry instead of leaving a scary backend-unavailable footer", async () => {
    let attempts = 0;
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        attempts += 1;
        if (attempts === 1) throw new Error("temporary bootstrap failure");
        return bootstrap;
      }
      return {};
    });

    render(<MixerSurface />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Mixer connection needs attention",
    );
    expect(screen.getByTestId("surface-footer")).not.toHaveTextContent(
      "Backend unavailable",
    );

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(screen.getAllByTestId("session-row")).toHaveLength(3));
    expect(attempts).toBe(2);
  });

  it("renders two distinct rows when sessions share the same id and name", async () => {
    // AudioSessionInfo ids are process ids: a single process (e.g. a
    // Chromium-style multi-stream process) can own several sessions with the
    // SAME id and the SAME display name. Row keys must stay unique or React
    // warns "Encountered two children with the same key" and mis-reconciles
    // on updates.
    const consoleErrors: string[] = [];
    const errorSpy = vi.spyOn(console, "error").mockImplementation(
      (...args: unknown[]) => {
        consoleErrors.push(args.map(String).join(" "));
      },
    );
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return {
          ...bootstrap,
          sessions: [
            { id: "42", name: "Chrome", pct: 30, muted: false, active: true },
            { id: "42", name: "Chrome", pct: 60, muted: false, active: false },
          ],
        };
      }
      return {};
    });
    render(<MixerSurface />);
    await screen.findAllByTestId("session-row");
    expect(screen.getAllByTestId("session-row")).toHaveLength(2);
    // Exercise an update pass (search re-render) so keyed reconciliation runs.
    fireEvent.change(screen.getByPlaceholderText(/search/i), {
      target: { value: "chr" },
    });
    expect(screen.getAllByTestId("session-row")).toHaveLength(2);
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: "" } });
    expect(screen.getAllByTestId("session-row")).toHaveLength(2);
    // The regression this test guards: the old `${id}-${name}` key was
    // identical for both rows and made React warn about duplicate keys.
    expect(
      consoleErrors.some((c) => c.includes("same key")),
      `unexpected React key warnings: ${JSON.stringify(consoleErrors)}`,
    ).toBe(false);
    errorSpy.mockRestore();
  });

  it("renders the system output row with the bootstrap volume", async () => {
    await renderSurface();
    expect(screen.getByTestId("system-output-row")).toHaveTextContent("System output");
    expect(screen.getByTestId("system-output-value")).toHaveTextContent("55%");
  });

  it("does not let a stale bootstrap overwrite a mute event", async () => {
    let resolveBootstrap!: (value: typeof bootstrap) => void;
    const pendingBootstrap = new Promise<typeof bootstrap>((resolve) => {
      resolveBootstrap = resolve;
    });
    vi.mocked(ipc.invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_bootstrap") return pendingBootstrap;
      return Promise.resolve({});
    });

    render(<MixerSurface />);
    await waitFor(() => expect(listeners["state://volume"]).toBeDefined());
    listeners["state://volume"]({ pct: 55, muted: true });
    resolveBootstrap(bootstrap);

    await waitFor(() => {
      expect(screen.getByTestId("system-output-value")).toHaveTextContent("Muted");
    });
  });

  it("shows a backend notice when system mute cannot reach the audio endpoint", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") return bootstrap;
      if (cmd === "toggle_mute") throw new Error("audio init failed: endpoint unavailable");
      return {};
    });

    await renderSurface();
    fireEvent.click(screen.getByRole("button", { name: "Mute system output" }));

    await waitFor(() => {
      expect(screen.getByRole("status")).toHaveTextContent(
        "Audio backend unavailable: audio init failed: endpoint unavailable",
      );
    });
    expect(screen.getByTestId("system-output-value")).toHaveTextContent("55%");
  });

  it("shows a live degraded notice from state://backend and clears it on ready", async () => {
    await renderSurface();
    await screen.findByText("55%");

    fire("state://backend", {
      status: "degraded",
      error: "audio device stopped responding",
    });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Audio backend temporarily unavailable: audio device stopped responding",
    );

    fire("state://backend", { status: "ready" });
    await waitFor(() =>
      expect(screen.queryByRole("status")).not.toBeInTheDocument(),
    );
  });

  it("shows the degraded notice immediately from a degraded bootstrap", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_bootstrap") {
        return {
          ...bootstrap,
          backend_status: { status: "degraded", error: "no audio endpoint" },
        };
      }
      return {};
    });

    await renderSurface();
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Audio backend temporarily unavailable: no audio endpoint",
    );
  });

  it("renders the glass-surface class on the root for transparent-window readability", async () => {
    await renderSurface();
    expect(screen.getByRole("main").className).toContain("glass-surface");
  });

  it("keeps the system row outside the bounded session scroll region", async () => {
    await renderSurface();
    const main = screen.getByRole("main");
    expect(main).toHaveAttribute("data-surface", "mixer");
    expect(screen.getByTestId("surface-header")).toBeInTheDocument();
    expect(screen.getByTestId("surface-content")).toBeInTheDocument();
    expect(screen.getByTestId("surface-footer")).toBeInTheDocument();
    const header = screen.getByTestId("surface-header");
    const content = screen.getByTestId("surface-content");
    expect(header).toContainElement(screen.getByTestId("system-output-row"));
    expect(content).not.toContainElement(screen.getByTestId("system-output-row"));
  });
});
