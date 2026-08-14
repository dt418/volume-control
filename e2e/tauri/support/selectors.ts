export type SurfaceName = "mixer" | "settings" | "help";

export const selectors = {
  surfaceRoot: (surface: SurfaceName) => `[data-surface="${surface}"]`,
  surfaceTitle: (surface: SurfaceName) => `${selectors.surfaceRoot(surface)} [data-surface-title]`,
  header: (surface: SurfaceName) => `${selectors.surfaceRoot(surface)} [data-surface-header]`,
  content: (surface: SurfaceName) => `${selectors.surfaceRoot(surface)} [data-surface-content]`,
  footer: (surface: SurfaceName) => `${selectors.surfaceRoot(surface)} [data-surface-footer]`,
  mixer: {
    search: '[data-testid="mixer-search"]',
    systemVolume: '[aria-label="System output volume"]',
    mute: '[aria-label="Mute"]',
    reset: '[aria-label="Reset volume to 50 percent"]',
  },
  settings: {
    save: '[data-testid="settings-save"]',
    reset: '[data-testid="settings-reset"]',
    cancel: '[data-testid="settings-cancel"]',
    status: '[aria-live="polite"]',
  },
  help: {
    search: '[data-testid="help-search"]',
    settings: '[data-testid="help-settings"]',
    close: '[data-testid="help-close"]',
  },
} as const;
