import { useT } from "../lib/i18n";

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
  const tr = useT();
  return (
    <footer data-testid="surface-footer" className="flex flex-shrink-0 items-center justify-end gap-2 border-t border-foreground/10 bg-background/90 px-4 py-2">
      <button
        type="button"
        onClick={onEditConfig}
        className="rounded-md border border-foreground/20 px-3 py-1.5 text-sm transition-colors hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
      >
        {tr("help.editConfig")}
      </button>
      <button
        type="button"
        onClick={onSettings}
        className="rounded-md border border-foreground/20 px-3 py-1.5 text-sm transition-colors hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
      >
        {tr("settings.title")}
      </button>
      <button
        type="button"
        onClick={onClose}
        className="rounded-md bg-accent px-3 py-1.5 text-sm font-medium text-accent-foreground transition-colors hover:bg-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
      >
        {tr("help.close")}
      </button>
    </footer>
  );
}
