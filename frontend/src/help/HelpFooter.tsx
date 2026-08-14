/** Sticky footer with the three legacy buttons (help.rs parity): `Edit
 *  config` → open_config_location, `Settings` → open the settings surface,
 *  `Close` → close the help surface. Tab order matches legacy. */
export function HelpFooter({
  onEditConfig,
  onSettings,
  onClose,
}: {
  onEditConfig: () => void;
  onSettings: () => void;
  onClose: () => void;
}) {
  return (
    <footer data-testid="surface-footer" className="flex flex-shrink-0 items-center justify-end gap-2 border-t border-foreground/10 bg-background/90 px-4 py-2">
      <button
        type="button"
        onClick={onEditConfig}
        className="rounded-md border border-foreground/20 px-3 py-1.5 text-sm"
      >
        Edit config
      </button>
      <button
        type="button"
        onClick={onSettings}
        className="rounded-md border border-foreground/20 px-3 py-1.5 text-sm"
      >
        Settings
      </button>
      <button
        type="button"
        onClick={onClose}
        className="rounded-md bg-accent px-3 py-1.5 text-sm font-medium text-accent-foreground"
      >
        Close
      </button>
    </footer>
  );
}
