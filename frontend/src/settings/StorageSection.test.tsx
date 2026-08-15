import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import * as ipc from "../lib/ipc";
import { StorageSection } from "./StorageSection";

vi.mock("../lib/ipc", () => ({
  invoke: vi.fn(async () => ""),
  listen: vi.fn(async () => () => {}),
}));

describe("StorageSection", () => {
  it("renders the config path from the config_path command", async () => {
    vi.mocked(ipc.invoke).mockResolvedValue("C:\\Users\\test\\AppData\\Roaming\\volumecontrol\\config.ini");
    render(<StorageSection />);
    expect(
      await screen.findByText(
        "C:\\Users\\test\\AppData\\Roaming\\volumecontrol\\config.ini",
      ),
    ).toBeInTheDocument();
    expect(ipc.invoke).toHaveBeenCalledWith("config_path");
  });

  it("Open config file invokes open_config_location", async () => {
    render(<StorageSection />);
    fireEvent.click(screen.getByRole("button", { name: /Open config\.ini \(advanced\)/i }));
    expect(ipc.invoke).toHaveBeenCalledWith("open_config_location");
  });

  it("shows the reload note", async () => {
    render(<StorageSection />);
    expect(
      await screen.findByText(/changes reload automatically/i),
    ).toBeInTheDocument();
  });

  it("explains the canonical INI file and legacy JSON backup", async () => {
    render(<StorageSection />);
    expect(
      await screen.findByText(/config\.ini directly.*config\.json.*migration backup/i),
    ).toBeInTheDocument();
  });

  it("shows a recovery warning supplied by bootstrap", async () => {
    render(<StorageSection notice="recovered_from_json" />);
    expect(
      await screen.findByRole("alert"),
    ).toHaveTextContent(/recovered.*legacy JSON backup/i);
  });
});
