import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { useT, type TranslationKey } from "../lib/i18n";
import { FieldError } from "./FieldError";
import type { DraftSetter, FieldErrors, SettingsConfig } from "./settingsTypes";

interface BeepField {
  key: "blocked_freq" | "blocked_duration_ms" | "limit_freq" | "limit_duration_ms";
  labelKey: TranslationKey;
  helpKey: TranslationKey;
  min: number;
  max: number;
}

const BEEP_FIELDS: BeepField[] = [
  {
    key: "blocked_freq",
    labelKey: "feedback.blockedFreq",
    helpKey: "feedback.blockedFreqHelp",
    min: 37,
    max: 32767,
  },
  {
    key: "blocked_duration_ms",
    labelKey: "feedback.blockedDuration",
    helpKey: "feedback.blockedDurationHelp",
    min: 10,
    max: 2000,
  },
  {
    key: "limit_freq",
    labelKey: "feedback.limitFreq",
    helpKey: "feedback.limitFreqHelp",
    min: 37,
    max: 32767,
  },
  {
    key: "limit_duration_ms",
    labelKey: "feedback.limitDuration",
    helpKey: "feedback.limitDurationHelp",
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
  const tr = useT();
  return (
    <Card>
      <CardHeader>
        <CardTitle>{tr("settings.feedback")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            aria-label={tr("feedback.beepEnabled")}
            checked={draft.beep.enabled}
            onChange={(e) =>
              setDraft((d) => ({ ...d, beep: { ...d.beep, enabled: e.target.checked } }))
            }
          />
          {tr("feedback.beepEnabled")}
        </label>
        {BEEP_FIELDS.map(({ key, labelKey, helpKey, min, max }) => {
          const full = `beep.${key}`;
          return (
            <label className="flex flex-col gap-1 text-sm" key={key}>
              <span>{tr(labelKey)}</span>
              <span className="text-xs text-foreground/60">{tr(helpKey)}</span>
              <Input
                aria-label={tr(labelKey)}
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
