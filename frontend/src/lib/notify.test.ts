import { beforeEach, describe, expect, it, vi } from "vitest";

const notification = vi.hoisted(() => ({
  isPermissionGranted: vi.fn(async () => false),
  requestPermission: vi.fn(
    async (): Promise<"granted" | "denied" | "prompt"> => "granted",
  ),
  sendNotification: vi.fn(),
}));

const ipc = vi.hoisted(() => ({ invoke: vi.fn(async () => true) }));

vi.mock("@tauri-apps/plugin-notification", () => notification);
vi.mock("../lib/ipc", () => ipc);

import { notifyUpdate } from "./notify";
import type { UpdateInfoPayload } from "./update";

const available: UpdateInfoPayload = {
  current_version: "v0.1.10",
  latest_version: "v0.1.11",
  is_update_available: true,
  release_url: "https://example.test/v0.1.11",
  message: "Update available: v0.1.11",
};

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    value: {},
    configurable: true,
  });
  notification.isPermissionGranted.mockResolvedValue(false);
  notification.requestPermission.mockResolvedValue("granted");
});

describe("notifyUpdate", () => {
  it("requests permission before sending the first update notification", async () => {
    await notifyUpdate(available);
    expect(ipc.invoke).toHaveBeenCalledWith("request_notification_permission");
    expect(notification.requestPermission).toHaveBeenCalledTimes(1);
    expect(notification.sendNotification).toHaveBeenCalledWith({
      title: "Volume Control",
      body: available.message,
    });
    await notifyUpdate(available);
    expect(notification.sendNotification).toHaveBeenCalledTimes(1);
  });

  it("does not send when permission is denied", async () => {
    notification.requestPermission.mockResolvedValue("denied");
    await notifyUpdate(available);
    expect(notification.sendNotification).not.toHaveBeenCalled();
    expect(localStorage.getItem("volumecontrol.notified-update")).toBeNull();
  });

  it("stops when the native permission request is denied", async () => {
    ipc.invoke.mockResolvedValueOnce(false);
    await notifyUpdate(available);
    expect(notification.isPermissionGranted).not.toHaveBeenCalled();
    expect(notification.sendNotification).not.toHaveBeenCalled();
  });

  it("does nothing outside Tauri or when no update is available", async () => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    await notifyUpdate(available);
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {},
      configurable: true,
    });
    await notifyUpdate({ ...available, is_update_available: false });
    expect(notification.isPermissionGranted).not.toHaveBeenCalled();
    expect(notification.sendNotification).not.toHaveBeenCalled();
  });
});
