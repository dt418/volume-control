import type { ColorThresholds } from "../mixer/SignalRail";

export interface HotkeyBindings {
  volume_up: string;
  volume_down: string;
  volume_up_large: string;
  volume_down_large: string;
  toggle_mute: string;
  reset_50: string;
  open_mixer: string;
  open_menu: string;
}

export interface SettingsConfig {
  volume_step: number;
  volume_step_large: number;
  overlay_duration_ms: number;
  /** Persisted preference; the live registry state is loaded separately. */
  autostart?: boolean;
  modifier: string;
  /** Older bootstrap fixtures/configs may omit this; the surface hydrates it. */
  hotkeys?: HotkeyBindings;
  blacklist: string[];
  color_thresholds: ColorThresholds;
  beep: {
    enabled: boolean;
    blocked_freq: number;
    blocked_duration_ms: number;
    limit_freq: number;
    limit_duration_ms: number;
  };
  appearance: {
    theme: string;
    material: string;
    motion: string;
    accent: string;
  };
}

export interface FieldErrors { [field: string]: string | undefined }
export type DraftSetter = (update: (draft: SettingsConfig) => SettingsConfig) => void;
