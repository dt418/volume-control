import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { UpdateCard } from "./UpdateCard";
import * as ipc from "../lib/ipc";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("UpdateCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders version display and check for updates button", () => {
    render(<UpdateCard />);
    expect(screen.getByText("Updates")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeInTheDocument();
  });

  it("invokes check_for_updates when clicking check button and renders available banner", async () => {
    vi.mocked(ipc.invoke).mockResolvedValueOnce({
      current_version: "0.1.7",
      latest_version: "v0.1.8",
      is_update_available: true,
      release_url: "https://github.com/dt418/volume-control/releases/tag/v0.1.8",
      message: "New version available: v0.1.8",
    });

    render(<UpdateCard />);
    const btn = screen.getByRole("button", { name: "Check for updates" });
    fireEvent.click(btn);

    await waitFor(() => {
      expect(ipc.invoke).toHaveBeenCalledWith("check_for_updates");
      expect(screen.getByText("New version available: v0.1.8")).toBeInTheDocument();
      expect(screen.getByRole("link", { name: "View release notes" })).toHaveAttribute(
        "href",
        "https://github.com/dt418/volume-control/releases/tag/v0.1.8"
      );
    });
  });

  it("renders up to date status when no update available", async () => {
    vi.mocked(ipc.invoke).mockResolvedValueOnce({
      current_version: "0.1.7",
      latest_version: "v0.1.7",
      is_update_available: false,
      message: "VolumeControl is up to date",
    });

    render(<UpdateCard />);
    const btn = screen.getByRole("button", { name: "Check for updates" });
    fireEvent.click(btn);

    await waitFor(() => {
      expect(screen.getByText("VolumeControl is up to date")).toBeInTheDocument();
    });
  });
});
