import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { UpdateCard } from "./UpdateCard";
import * as ipc from "../lib/ipc";
import pkg from "../../package.json";
import type { UpdateStatePayload } from "../lib/update";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("UpdateCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_update_state") {
        return {
          revision: 0,
          phase: "idle",
          info: null,
          error: null,
          install_supported: true,
        } as UpdateStatePayload;
      }
      return {};
    });
  });

  it("shows package version before any update check", () => {
    render(<UpdateCard />);
    expect(screen.getByText(`v${pkg.version}`)).toBeInTheDocument();
  });

  it("renders check for updates button", () => {
    render(<UpdateCard />);
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeInTheDocument();
  });

  it("invokes check_for_updates when clicking check button and renders available/downloading state", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "check_for_updates") {
        return {
          revision: 1,
          phase: "available",
          info: {
            current_version: "0.1.7",
            latest_version: "v0.1.8",
            is_update_available: true,
            release_url: "https://github.com/dt418/volume-control/releases/tag/v0.1.8",
            message: "New version available: v0.1.8",
          },
          error: null,
          install_supported: true,
        } as UpdateStatePayload;
      }
      return {
        revision: 0,
        phase: "idle",
        info: null,
        error: null,
        install_supported: true,
      };
    });

    render(<UpdateCard />);
    const btn = screen.getByRole("button", { name: "Check for updates" });
    fireEvent.click(btn);

    await waitFor(() => {
      expect(ipc.invoke).toHaveBeenCalledWith("check_for_updates", {
        force: true,
      });
      expect(screen.getByText("New version available: v0.1.8")).toBeInTheDocument();
      expect(screen.getByText("Downloading update in background…")).toBeInTheDocument();
      const link = screen.getByRole("link", { name: "View release notes" });
      expect(link).toHaveAttribute(
        "href",
        "https://github.com/dt418/volume-control/releases/tag/v0.1.8"
      );
      fireEvent.click(link);
      expect(ipc.invoke).toHaveBeenCalledWith("open_release", {
        url: "https://github.com/dt418/volume-control/releases/tag/v0.1.8",
      });
    });
  });

  it("renders ready state with confirmation dialog and invokes install_update", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_update_state") {
        return {
          revision: 2,
          phase: "ready",
          info: {
            current_version: "0.1.10",
            latest_version: "v0.1.11",
            is_update_available: true,
            release_url: "https://github.com/dt418/volume-control/releases/tag/v0.1.11",
            message: "Ready to install",
          },
          error: null,
          install_supported: true,
        } as UpdateStatePayload;
      }
      if (cmd === "install_update") {
        return undefined;
      }
      return {};
    });

    render(<UpdateCard />);

    const installBtn = await screen.findByRole("button", { name: "Install and restart" });
    expect(installBtn).toBeInTheDocument();

    // Click "Install and restart" opens confirmation panel
    fireEvent.click(installBtn);

    expect(screen.getByText(/VolumeControl will restart to apply version v0.1.11/i)).toBeInTheDocument();
    const confirmBtn = screen.getByRole("button", { name: "Confirm installation" });
    const laterBtn = screen.getByRole("button", { name: "Later" });

    // Clicking Later cancels
    fireEvent.click(laterBtn);
    expect(screen.queryByText(/VolumeControl will restart to apply version v0.1.11/i)).not.toBeInTheDocument();

    // Open again and confirm
    fireEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm installation" }));

    await waitFor(() => {
      expect(ipc.invoke).toHaveBeenCalledWith("install_update", {
        expected_version: "v0.1.11",
      });
    });
  });

  it("renders up to date status when no update available", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "check_for_updates") {
        return {
          revision: 1,
          phase: "idle",
          info: {
            current_version: "0.1.10",
            latest_version: "v0.1.10",
            is_update_available: false,
            release_url: null,
            message: "Volume Control is up to date",
          },
          error: null,
          install_supported: true,
        } as UpdateStatePayload;
      }
      return {
        revision: 0,
        phase: "idle",
        info: null,
        error: null,
        install_supported: true,
      };
    });

    render(<UpdateCard />);
    const btn = screen.getByRole("button", { name: "Check for updates" });
    fireEvent.click(btn);

    await waitFor(() => {
      expect(screen.getByText("Volume Control is up to date")).toBeInTheDocument();
    });
  });

  it("renders error state and retries on click", async () => {
    vi.mocked(ipc.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_update_state") {
        return {
          revision: 1,
          phase: "error",
          info: null,
          error: "network offline",
          install_supported: true,
        } as UpdateStatePayload;
      }
      if (cmd === "check_for_updates") {
        return {
          revision: 2,
          phase: "idle",
          info: {
            current_version: "0.1.10",
            latest_version: "v0.1.10",
            is_update_available: false,
            release_url: null,
            message: "Volume Control is up to date",
          },
          error: null,
          install_supported: true,
        } as UpdateStatePayload;
      }
      return {};
    });

    render(<UpdateCard />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Update failed: network offline");

    const retryBtn = screen.getByRole("button", { name: "Retry check" });
    fireEvent.click(retryBtn);

    await waitFor(() => {
      expect(ipc.invoke).toHaveBeenCalledWith("check_for_updates", { force: true });
    });
  });
});
