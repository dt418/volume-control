import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";

import * as ipc from "../lib/ipc";
import { MixerSurface } from "./MixerSurface";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(),
  listen: vi.fn(async () => () => {}),
}));

// Captured `listen` handlers keyed by event name (reset in beforeEach).
const listeners: Record<string, (payload: unknown) => void> = {};

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

  it("closes the window on Escape", async () => {
    await renderSurface();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(ipc.invoke).toHaveBeenCalledWith("close_surface", {
      surface: "window-mixer",
    });
  });

  it("closes the window when the Rust side emits close_mixer_request (blur auto-close)", async () => {
    await renderSurface();
    // The surface subscribes to the Rust Focused(false) event.
    expect(listeners["close_mixer_request"]).toBeDefined();
    listeners["close_mixer_request"](null);
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
