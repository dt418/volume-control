export type SurfaceName = "mixer" | "settings" | "help";

export const selectors = {
  surfaceRoot: (surface: SurfaceName) => `[data-surface="${surface}"]`,
  surfaceTitle: (surface: SurfaceName) => `${selectors.surfaceRoot(surface)} h1`,
  header: (surface: SurfaceName) => `${selectors.surfaceRoot(surface)} [data-testid="surface-header"]`,
  content: (surface: SurfaceName) => `${selectors.surfaceRoot(surface)} [data-testid="surface-content"]`,
  footer: (surface: SurfaceName) => `${selectors.surfaceRoot(surface)} [data-testid="surface-footer"]`,
  mixer: {
    search: '[data-surface="mixer"] input[placeholder="Search apps…"]',
    systemVolume: '[aria-label="System output volume"]',
    mute: '[aria-label="Mute"]',
    reset: '[aria-label="Reset volume to 50 percent"]',
  },
  settings: {
    save: '[data-surface="settings"] button*=Save changes',
    reset: '[data-surface="settings"] button*=Reset',
    cancel: '[data-surface="settings"] button*=Cancel',
    status: '[aria-live="polite"]',
  },
  help: {
    search: '[data-surface="help"] input[aria-label="Search shortcuts"]',
    settings: '[data-surface="help"] button*=Settings',
    close: '[data-surface="help"] button[aria-label="Close"]',
  },
} as const;
