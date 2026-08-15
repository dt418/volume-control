import { setTheme } from "@tauri-apps/api/app";

/**
 * Appearance tokens pushed by Rust (AppCore::bootstrap / AppearancePayload).
 * theme_resolved is the adaptive outcome ("dark" | "light") of the config
 * theme + system preference; material/motion/accent mirror the config.
 */
export interface AppearancePayload {
  theme_resolved: string;
  material: string;
  motion: string;
  accent: string;
}

const THEME_KEY = "app-theme";

/**
 * Apply the Rust-resolved appearance to the document so every surface is
 * readable in the configured (and system-adaptive) theme. Keeps the FOUC
 * `<head>` script and this applier in sync via localStorage["app-theme"].
 * Also pushes the theme to the Tauri/WebView2 native theme API (requires the
 * `core:app:allow-set-theme` capability) so the preferred color scheme of
 * form controls/scrollbars matches the app theme.
 */
export function applyAppearance(appearance: AppearancePayload): void {
  const root = document.documentElement;
  const dark = appearance.theme_resolved === "dark";
  root.dataset.theme = dark ? "dark" : "light";
  root.classList.toggle("dark", dark);
  root.classList.toggle("reduced-motion", appearance.motion === "Reduced");
  try {
    localStorage.setItem(THEME_KEY, dark ? "dark" : "light");
  } catch {
    // storage can be unavailable (private mode); the classList is enough.
  }
  // Best-effort native sync; the CSS classes remain the render source of
  // truth. bootstrap's theme_resolved is authoritative (Rust already folded
  // the system preference in), so we do not race the OS theme here.
  void setTheme(dark ? "dark" : "light").catch(() => {});
}
