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
    mute: '[aria-label="Mute system output"]',
    reset: '[aria-label="Reset volume to 50%"]',
  },
  settings: {
    save: '//main[@data-surface="settings"]//button[normalize-space()="Save changes"]',
    reset: '//main[@data-surface="settings"]//button[normalize-space()="Reset"]',
    cancel: '//main[@data-surface="settings"]//button[normalize-space()="Cancel"]',
    status: '[aria-live="polite"]',
  },
  help: {
    search: '[data-surface="help"] input[aria-label="Search shortcuts"]',
    settings: '//main[@data-surface="help"]//button[normalize-space()="Settings"]',
    close: '[data-surface="help"] button[aria-label="Close"]',
  },
} as const;
