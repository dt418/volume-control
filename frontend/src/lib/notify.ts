import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

import { invoke } from "./ipc";
import type { UpdateInfoPayload } from "./update";

const NOTIFIED_VERSION_KEY = "volumecontrol.notified-update";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// Multiple surfaces can receive the same update result at once; share one
// permission/send pass per page load.
let inFlight: Promise<void> | null = null;

/**
 * Show one native OS notification per available release.
 *
 * Permission is requested lazily, only when an update exists: macOS presents
 * a user prompt, Windows/Linux report their current notification state. A
 * denied permission is left unrecorded so the next app run can ask again
 * after the user changes the OS setting.
 */
export function notifyUpdate(info: UpdateInfoPayload): Promise<void> {
  if (!info.is_update_available || !inTauri()) return Promise.resolve();
  const version = info.latest_version ?? "";
  if (!version) return Promise.resolve();
  if (inFlight) return inFlight;
  inFlight = showOnce(info, version).finally(() => {
    inFlight = null;
  });
  return inFlight;
}

async function showOnce(info: UpdateInfoPayload, version: string): Promise<void> {
  try {
    if (localStorage.getItem(NOTIFIED_VERSION_KEY) === version) return;
    // macOS has a real user prompt; the Tauri desktop plugin cannot request
    // it, so go through the native command first. Other platforms return true.
    const nativeGranted = await invoke<boolean>(
      "request_notification_permission",
    ).catch(() => true);
    if (!nativeGranted) return;
    let granted = await isPermissionGranted();
    if (!granted) {
      granted = (await requestPermission()) === "granted";
    }
    if (!granted) return;
    sendNotification({
      title: "Volume Control",
      body: info.message,
    });
    localStorage.setItem(NOTIFIED_VERSION_KEY, version);
  } catch {
    // Notifications are best-effort; a missing permission/backend must never
    // break the update banner or Settings surface.
  }
}
