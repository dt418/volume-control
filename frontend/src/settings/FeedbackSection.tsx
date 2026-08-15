import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { FieldError } from "./FieldError";
import type { DraftSetter, FieldErrors, SettingsConfig } from "./settingsTypes";

interface BeepField {
  key: "blocked_freq" | "blocked_duration_ms" | "limit_freq" | "limit_duration_ms";
  label: string;
  help: string;
  min: number;
  max: number;
}

const BEEP_FIELDS: BeepField[] = [
  {
    key: "blocked_freq",
    label: "Blocked beep frequency",
    help: "Beep when a shortcut is blocked.",
    min: 37,
    max: 32767,
  },
  {
    key: "blocked_duration_ms",
    label: "Blocked beep duration",
    help: "How long the blocked beep sounds.",
    min: 10,
    max: 2000,
  },
  {
    key: "limit_freq",
    label: "Limit beep frequency",
    help: "Beep when volume is already at the limit.",
    min: 37,
    max: 32767,
  },
  {
    key: "limit_duration_ms",
    label: "Limit beep duration",
    help: "How long the limit beep sounds.",
    min: 10,
    max: 2000,
  },
];

/** Feedback section (legacy parity): beep master switch + the four blocked /
 *  limit frequency/duration inputs with inline field errors. */
export function FeedbackSection({
  draft,
  setDraft,
  errors,
}: {
  draft: SettingsConfig;
  setDraft: DraftSetter;
  errors: FieldErrors;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Feedback</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            aria-label="Enable beep feedback"
            checked={draft.beep.enabled}
            onChange={(e) =>
              setDraft((d) => ({ ...d, beep: { ...d.beep, enabled: e.target.checked } }))
            }
          />
          Enable beep feedback
        </label>
        {BEEP_FIELDS.map(({ key, label, help, min, max }) => {
          const full = `beep.${key}`;
          return (
            <label className="flex flex-col gap-1 text-sm" key={key}>
              <span>{label}</span>
              <span className="text-xs text-foreground/60">{help}</span>
              <Input
                aria-label={label}
                type="number"
                min={min}
                max={max}
                value={draft.beep[key]}
                aria-invalid={Boolean(errors[full])}
                onChange={(e) =>
                  setDraft((d) => ({
                    ...d,
                    beep: { ...d.beep, [key]: Number(e.target.value) },
                  }))
                }
              />
              <FieldError message={errors[full]} />
            </label>
          );
        })}
      </CardContent>
    </Card>
  );
}
