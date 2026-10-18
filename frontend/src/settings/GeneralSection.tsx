import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { useT, type TranslationKey } from "../lib/i18n";
import { AutoStartControl } from "./AutoStartControl";
import { FieldError } from "./FieldError";
import type { DraftSetter, FieldErrors, SettingsConfig } from "./settingsTypes";
import { UpdateCard } from "./UpdateCard";

interface GeneralField {
  key: "volume_step" | "volume_step_large" | "overlay_duration_ms";
  labelKey: TranslationKey;
  helpKey: TranslationKey;
  min: number;
  max: number;
}

const GENERAL_FIELDS: GeneralField[] = [
  {
    key: "volume_step",
    labelKey: "settings.volumeStep",
    helpKey: "settings.volumeStepHelp",
    min: 1,
    max: 50,
  },
  {
    key: "volume_step_large",
    labelKey: "settings.volumeStepLarge",
    helpKey: "settings.volumeStepLargeHelp",
    min: 1,
    max: 50,
  },
  {
    key: "overlay_duration_ms",
    labelKey: "settings.overlayDuration",
    helpKey: "settings.overlayDurationHelp",
    min: 200,
    max: 10000,
  },
];

/** General section (legacy parity): step sizes + overlay duration with
 *  inline field errors, plus the UI language selector. */
export function GeneralSection({
  draft,
  setDraft,
  errors,
  autoStartEnabled = false,
  autoStartDisabled = true,
  onAutoStartChange = async () => {},
}: {
  draft: SettingsConfig;
  setDraft: DraftSetter;
  errors: FieldErrors;
  autoStartEnabled?: boolean;
  autoStartDisabled?: boolean;
  onAutoStartChange?: (enabled: boolean) => void;
}) {
  const tr = useT();
  return (
    <div className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>{tr("settings.general")}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <label className="flex flex-col gap-1 text-sm">
            <span>{tr("app.language")}</span>
            <select
              aria-label={tr("app.language")}
              value={draft.language ?? "en"}
              onChange={(e) => setDraft((d) => ({ ...d, language: e.target.value }))}
              className="rounded-md border border-foreground/20 bg-background px-2 py-1.5 text-sm"
            >
              <option value="en">{tr("app.languageEn")}</option>
              <option value="vi">{tr("app.languageVi")}</option>
            </select>
          </label>
          {GENERAL_FIELDS.map(({ key, labelKey, helpKey, min, max }) => (
            <label className="flex flex-col gap-1 text-sm" key={key}>
              <span>{tr(labelKey)}</span>
              <span className="text-xs text-foreground/60">{tr(helpKey)}</span>
              <Input
                aria-label={tr(labelKey)}
                type="number"
                min={min}
                max={max}
                value={draft[key]}
                aria-invalid={Boolean(errors[key])}
                onChange={(e) =>
                  setDraft((d) => ({ ...d, [key]: Number(e.target.value) }))
                }
              />
              <FieldError message={errors[key]} />
            </label>
          ))}
          <AutoStartControl
            enabled={autoStartEnabled}
            disabled={autoStartDisabled}
            onChange={onAutoStartChange}
          />
        </CardContent>
      </Card>
      <UpdateCard />
    </div>
  );
}
