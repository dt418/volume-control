import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import * as ipc from "../lib/ipc";
import { SystemOutputRow } from "./SystemOutputRow";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(),
}));

const thresholds = { green_up_to: 40, blue_up_to: 75, orange_up_to: 100 };

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.invoke).mockResolvedValue({});
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("SystemOutputRow", () => {
  it("renders the live volume value", () => {
    render(<SystemOutputRow value={55} muted={false} thresholds={thresholds} />);
    expect(screen.getByTestId("system-output-value")).toHaveTextContent("55%");
  });

  it("renders Muted instead of a percent when muted", () => {
    render(<SystemOutputRow value={55} muted={true} thresholds={thresholds} />);
    expect(screen.getByTestId("system-output-value")).toHaveTextContent("Muted");
    // The value label and the mute control both carry the muted state.
    expect(screen.getByRole("button", { name: "Unmute system output" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("invokes toggle_mute when the Mute button is clicked", () => {
    render(<SystemOutputRow value={55} muted={false} thresholds={thresholds} />);
    fireEvent.click(screen.getByRole("button", { name: "Mute system output" }));
    expect(ipc.invoke).toHaveBeenCalledWith("toggle_mute");
  });

  it("shows Unmute when muted and invokes toggle_mute", () => {
    render(<SystemOutputRow value={55} muted={true} thresholds={thresholds} />);
    const unmute = screen.getByRole("button", { name: "Unmute system output" });
    expect(unmute).toBeInTheDocument();
    fireEvent.click(unmute);
    expect(ipc.invoke).toHaveBeenCalledWith("toggle_mute");
  });

  it("invokes reset_volume when the Reset button is clicked", () => {
    render(<SystemOutputRow value={55} muted={false} thresholds={thresholds} />);
    fireEvent.click(screen.getByRole("button", { name: "Reset volume to 50%" }));
    expect(ipc.invoke).toHaveBeenCalledWith("reset_volume");
  });

  it("invokes set_volume when the system slider changes", () => {
    render(<SystemOutputRow value={55} muted={false} thresholds={thresholds} />);
    const slider = screen.getByRole("slider", { name: "System output volume" });
    fireEvent.keyDown(slider, { key: "ArrowRight" }); // Radix slider step
    expect(ipc.invoke).toHaveBeenCalledWith("set_volume", { percent: 56 });
  });

  it("renders with CardSm high-density styling and no nested blur", () => {
    render(<SystemOutputRow value={55} muted={false} thresholds={thresholds} />);
    const row = screen.getByTestId("system-output-row");
    expect(row.className).toContain("rounded-xl");
    expect(row.className).toContain("bg-card/60");
    expect(row.className).not.toContain("backdrop-blur");
  });
});
