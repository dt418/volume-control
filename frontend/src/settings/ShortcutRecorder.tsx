import { useEffect, useRef, useState } from "react";

import { cn } from "../lib/utils";
import { Kbd } from "../components/ui/kbd";
import { formatShortcut } from "./modifierOptions";

export interface ShortcutRecorderProps {
  id?: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  "aria-label"?: string;
}

function isModifierKey(key: string): boolean {
  return key === "Control" || key === "Shift" || key === "Alt" || key === "Meta";
}

function recordableShortcut(event: KeyboardEvent): string | null {
  if (isModifierKey(event.key) || !event.code || event.code === "Unidentified") return null;
  const modifiers = [
    event.ctrlKey ? "Ctrl" : null,
    event.altKey ? "Alt" : null,
    event.shiftKey ? "Shift" : null,
    event.metaKey ? "Cmd" : null,
  ].filter((modifier): modifier is string => modifier !== null);
  if (modifiers.length === 0) return null;
  return [...modifiers, event.code].join("+");
}

/** Screenshot-style global shortcut field with an actual keyboard recorder. */
export function ShortcutRecorder({
  id,
  value,
  onChange,
  disabled = false,
  "aria-label": ariaLabel,
}: ShortcutRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const captureRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (recording) captureRef.current?.focus();
  }, [recording]);

  useEffect(() => {
    if (!recording) return undefined;
    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        setRecording(false);
        setMessage(null);
        return;
      }
      if (event.key === "Backspace" || event.key === "Delete") {
        onChange("");
        setRecording(false);
        setMessage("Shortcut cleared");
        return;
      }
      if (event.repeat) return;
      const shortcut = recordableShortcut(event);
      if (!shortcut) {
        if (!isModifierKey(event.key)) {
          setMessage("Press a modifier such as Ctrl, Alt, Shift, or Cmd with a key");
        }
        return;
      }
      onChange(shortcut);
      setRecording(false);
      setMessage("Shortcut recorded");
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [onChange, recording]);

  const display = value ? formatShortcut(value) : "Not set";
  const label = ariaLabel ?? "Global shortcut";

  return (
    <div
      className="flex min-w-0 items-center gap-2"
      data-testid="shortcut-recorder"
      data-shortcut-action={id}
    >
      <button
        ref={captureRef}
        type="button"
        disabled={disabled}
        aria-label={`${label}: ${display}`}
        aria-pressed={recording}
        onClick={() => {
          setMessage(null);
          setRecording(true);
        }}
        className={cn(
          "min-w-[10rem] flex-1 rounded-md border px-3 py-2 text-center text-sm font-medium",
          "border-foreground/20 bg-background/60 text-foreground",
          "hover:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70",
          recording && "border-accent bg-accent/10 text-accent",
          disabled && "cursor-not-allowed opacity-50",
        )}
      >
        <Kbd className="border-0 bg-transparent px-0 py-0 text-inherit">
          {recording ? "Press shortcut…" : display}
        </Kbd>
      </button>
      <button
        type="button"
        disabled={disabled}
        aria-label={`Record ${label}`}
        data-testid={id ? `shortcut-record-${id}` : undefined}
        onClick={() => {
          setMessage(null);
          setRecording(true);
        }}
        className={cn(
          "rounded-md border border-foreground/20 px-3 py-2 text-sm",
          "hover:border-accent hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70",
          disabled && "cursor-not-allowed opacity-50",
        )}
      >
        Record
      </button>
      <button
        type="button"
        disabled={disabled || !value}
        aria-label={`Clear ${label}`}
        data-testid={id ? `shortcut-clear-${id}` : undefined}
        onClick={() => {
          setRecording(false);
          onChange("");
          setMessage("Shortcut cleared");
        }}
        className={cn(
          "rounded-md border border-foreground/20 px-3 py-2 text-sm",
          "hover:border-accent hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70",
          "disabled:cursor-not-allowed disabled:opacity-40",
        )}
      >
        Clear
      </button>
      <span className="sr-only" aria-live="polite">
        {message ?? ""}
      </span>
    </div>
  );
}

export { recordableShortcut };
