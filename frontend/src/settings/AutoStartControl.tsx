export interface AutoStartControlProps {
  enabled: boolean;
  disabled?: boolean;
  onChange: (enabled: boolean) => void;
}

/** Draft-backed auto-start control.
 *
 * Toggling only edits the Settings draft; the registry write happens when the
 * user presses "Save changes" (same lifecycle as every other setting). The
 * host still surfaces platform unavailability through `disabled`.
 */
export function AutoStartControl({
  enabled,
  disabled = false,
  onChange,
}: AutoStartControlProps) {
  return (
    <div className="flex flex-col gap-2 border-t border-foreground/10 pt-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium">Start with Windows</span>
          <span className="text-xs text-foreground/60">
            Launch VolumeControl when you sign in.
          </span>
        </div>
        <button
          type="button"
          role="switch"
          aria-label="Start with Windows"
          aria-checked={enabled}
          disabled={disabled}
          onClick={() => onChange(!enabled)}
          className="relative h-6 w-11 rounded-full border border-foreground/25 bg-foreground/10 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent aria-checked:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
        >
          <span
            aria-hidden="true"
            className={`absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-background shadow-sm transition-transform ${
              enabled ? "translate-x-5" : "translate-x-0"
            }`}
          />
        </button>
      </div>
      {disabled && (
        <p role="status" className="text-xs text-foreground/60">
          Auto-start is unavailable on this platform.
        </p>
      )}
    </div>
  );
}
