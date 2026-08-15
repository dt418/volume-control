import { cn } from "../lib/utils";
import { MODIFIER_OPTIONS } from "./modifierOptions";

export interface ModifierPickerProps {
  selected: string;
  onSelect: (id: string) => void;
}

/** Compatibility presets for users who prefer the original modifier layouts. */
export function ModifierPicker({ selected, onSelect }: ModifierPickerProps) {
  return (
    <div className="flex flex-wrap gap-2" role="group" aria-label="Shortcut presets">
      {MODIFIER_OPTIONS.map((option) => {
        const active = selected === option.id;
        return (
          <button
            key={option.id}
            type="button"
            disabled={option.disabled}
            title={option.disabled ? option.disabledReason : undefined}
            aria-pressed={active}
            onClick={() => !option.disabled && onSelect(option.id)}
            className={cn(
              "rounded-md border px-3 py-1.5 text-sm transition-colors",
              "border-foreground/20 bg-background/60 text-foreground",
              "hover:border-accent hover:text-accent",
              "disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-foreground/20 disabled:hover:text-foreground",
              active &&
                "border-accent bg-accent/15 text-accent",
            )}
          >
            {option.label}
          </button>
        );
      })}
      <span className="text-xs text-foreground/60" data-testid="capslock-note">
        Caps Lock falls back to Ctrl + Alt
      </span>
    </div>
  );
}
