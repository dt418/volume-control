import { useState } from "react";

export interface AutoStartControlProps {
  enabled: boolean;
  disabled?: boolean;
  onChange: (enabled: boolean) => Promise<void>;
}

/** Immediate, registry-backed auto-start control.
 *
 * Auto-start is intentionally not part of the draft Save/Reset lifecycle:
 * toggling it changes the current-user startup registration immediately, and
 * a rejected write leaves the last confirmed switch state intact.
 */
export function AutoStartControl({
  enabled,
  disabled = false,
  onChange,
}: AutoStartControlProps) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retryValue, setRetryValue] = useState<boolean | null>(null);

  const commit = async (next: boolean) => {
    setPending(true);
    setError(null);
    setRetryValue(null);
    try {
      await onChange(next);
    } catch (reason) {
      setError(String(reason));
      setRetryValue(next);
    } finally {
      setPending(false);
    }
  };

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
          disabled={disabled || pending}
          onClick={() => void commit(!enabled)}
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
      {error && (
        <div role="alert" className="flex items-center justify-between gap-2 text-xs text-destructive">
          <span>{error}</span>
          <button
            type="button"
            className="shrink-0 rounded-md border border-destructive/40 px-2 py-1"
            onClick={() => retryValue !== null && void commit(retryValue)}
            disabled={pending || retryValue === null}
          >
            Retry
          </button>
        </div>
      )}
    </div>
  );
}
