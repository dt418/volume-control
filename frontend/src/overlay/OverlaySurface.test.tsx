import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import { OverlaySurface } from "./OverlaySurface";

const listeners: Record<string, (payload: unknown) => void> = {};

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(),
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

const basePayload = {
  text: null,
  pct: 42,
  muted: false,
  green_up_to: 40,
  blue_up_to: 75,
  orange_up_to: 100,
  theme: "Dark",
  material: "Auto",
  motion: "Full",
  accent: "System",
};

describe("OverlaySurface", () => {
  it("renders the volume rail and percent from state://overlay", async () => {
    render(<OverlaySurface />);
    fire("state://overlay", basePayload);
    expect(await screen.findByText("42%")).toBeInTheDocument();
    expect(screen.getByTestId("signal-rail")).toBeInTheDocument();
  });

  it("renders a text card instead of the rail when text is present", async () => {
    render(<OverlaySurface />);
    fire("state://overlay", { ...basePayload, text: "Config reloaded" });
    expect(await screen.findByText("Config reloaded")).toBeInTheDocument();
  });

  it("renders Muted state from the rail", async () => {
    render(<OverlaySurface />);
    fire("state://overlay", { ...basePayload, pct: 0, muted: true });
    expect(await screen.findByText("Muted")).toBeInTheDocument();
  });
});
