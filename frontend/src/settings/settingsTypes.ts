import type { ColorThresholds } from "../mixer/SignalRail";

export interface SettingsConfig {
  volume_step: number;
  volume_step_large: number;
  overlay_duration_ms: number;
  modifier: string;
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
