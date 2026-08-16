import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";

import { OverlaySurface } from "./OverlaySurface";
import * as ipc from "../lib/ipc";

const listeners: Record<string, (payload: unknown) => void> = {};

const { closeOverlayWindow } = vi.hoisted(() => ({ closeOverlayWindow: vi.fn() }));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ close: closeOverlayWindow }),
}));

const bootstrap = {
  volume_pct: 55,
  muted: false,
  config: {
    color_thresholds: { green_up_to: 40, blue_up_to: 75, orange_up_to: 100 },
    overlay_duration_ms: 1800,
  },
  appearance: { theme_resolved: "dark", material: "Auto", motion: "Full", accent: "System" },
};

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(async () => bootstrap),
  listen: vi.fn(async (event: string, cb: (p: unknown) => void) => {
    listeners[event] = cb;
    return () => {};
  }),
}));

vi.mock("../lib/surface", () => ({
  markSurfaceReady: () => Promise.resolve(),
}));

afterEach(() => {
  Object.keys(listeners).forEach((key) => delete listeners[key]);
});

function fire(event: string, payload: unknown) {
  listeners[event]?.(payload);
}

describe("OverlaySurface", () => {
  it("renders the bootstrap state as soon as it mounts (sync fix)", async () => {
    render(<OverlaySurface />);
    expect(await screen.findByText("55%")).toBeInTheDocument();
    expect(screen.getByTestId("signal-rail")).toBeInTheDocument();
  });

  it("updates live from state://volume", async () => {
    render(<OverlaySurface />);
    await screen.findByText("55%");
    fire("state://volume", { pct: 62, muted: false });
    expect(await screen.findByText("62%")).toBeInTheDocument();
  });

  it("renders a text card from state://overlay", async () => {
    render(<OverlaySurface />);
    await screen.findByText("55%");
    fire("state://overlay", {
      text: "Config reloaded",
      pct: 50,
      muted: false,
      green_up_to: 40,
      blue_up_to: 75,
      orange_up_to: 100,
      theme_resolved: "dark",
      material: "Auto",
      motion: "Full",
      accent: "System",
    });
    expect(await screen.findByText("Config reloaded")).toBeInTheDocument();
  });

  it("renders Muted from the live volume event", async () => {
    render(<OverlaySurface />);
    await screen.findByText("55%");
    fire("state://volume", { pct: 0, muted: true });
    expect(await screen.findByText("Muted")).toBeInTheDocument();
  });

  describe("auto-hide self-timer", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
      vi.clearAllMocks();
    });

    it("does NOT arm the timer on the verify-marker mount (no payload)", async () => {
      render(<OverlaySurface />);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(screen.getByText("55%")).toBeInTheDocument();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(10_000);
      });
      // The debug/E2E marker path mounts without state://overlay; the
      // window must stay open so WebDriver can attach.
      expect(closeOverlayWindow).not.toHaveBeenCalled();
    });

    it("keeps the HUD open during debug E2E runs (e2e_debug flag)", async () => {
      vi.mocked(ipc.invoke).mockImplementationOnce(async () => ({
        ...bootstrap,
        e2e_debug: true,
      }));
      render(<OverlaySurface />);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      fire("state://overlay", {
        text: null,
        pct: 55,
        muted: false,
        green_up_to: 40,
        blue_up_to: 75,
        orange_up_to: 100,
        theme_resolved: "dark",
        material: "Auto",
        motion: "Full",
        accent: "System",
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(10_000);
      });
      expect(closeOverlayWindow).not.toHaveBeenCalled();
    });

    it("closes the window after the configured duration once a payload arrives", async () => {
      render(<OverlaySurface />);

      // Flush the async bootstrap (microtasks only) without firing the timer.
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(screen.getByText("55%")).toBeInTheDocument();
      expect(closeOverlayWindow).not.toHaveBeenCalled();

      fire("state://overlay", {
        text: null,
        pct: 55,
        muted: false,
        green_up_to: 40,
        blue_up_to: 75,
        orange_up_to: 100,
        theme_resolved: "dark",
        material: "Auto",
        motion: "Full",
        accent: "System",
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1_800);
      });
      expect(closeOverlayWindow).toHaveBeenCalledTimes(1);
    });
  });
});
