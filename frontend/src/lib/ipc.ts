import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(cmd, args);
}

export function listen<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  return tauriListen<T>(event, (e) => handler(e.payload));
}

/** The recommended blacklist presets for the current modifier (read-only;
 *  feeds the Blacklist editor's "Apply Recommended" draft merge). */
export function recommendedBlacklist(): Promise<string[]> {
  return invoke<string[]>("recommended_blacklist");
}

/** The on-disk config path shown by the Storage section. */
export function getConfigPath(): Promise<string> {
  return invoke<string>("config_path");
}

/** Open the config file in the default editor (host shows the "Editing
 *  config — changes reload automatically" overlay). */
export function openConfigLocation(): Promise<void> {
  return invoke<void>("open_config_location");
}
