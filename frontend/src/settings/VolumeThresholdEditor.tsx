import { Input } from "../components/ui/input";
import { useT, type TranslationKey } from "../lib/i18n";
import { FieldError } from "./FieldError";
import type { DraftSetter, FieldErrors, SettingsConfig } from "./settingsTypes";

type ThresholdKey = "green_up_to" | "blue_up_to" | "orange_up_to";

const THRESHOLD_FIELDS: Array<{ key: ThresholdKey; labelKey: TranslationKey }> = [
  { key: "green_up_to", labelKey: "threshold.green" },
  { key: "blue_up_to", labelKey: "threshold.blue" },
  { key: "orange_up_to", labelKey: "threshold.orange" },
];

/** Volume color thresholds (0–100%, monotonic order enforced by the backend;
 *  inline errors surface from the Save response field names). */
export function VolumeThresholdEditor({
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
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-semibold">{tr("threshold.heading")}</h3>
      {THRESHOLD_FIELDS.map(({ key, labelKey }) => {
        const full = `color_thresholds.${key}`;
        return (
          <label className="flex flex-col gap-1 text-sm" key={key}>
            <span>{tr(labelKey)}</span>
            <Input
              aria-label={tr(labelKey)}
              type="number"
              min={0}
              max={100}
              value={draft.color_thresholds[key]}
              aria-invalid={Boolean(errors[full])}
              onChange={(e) =>
                setDraft((d) => ({
                  ...d,
                  color_thresholds: {
                    ...d.color_thresholds,
                    [key]: Number(e.target.value),
                  },
                }))
              }
            />
            <FieldError message={errors[full]} />
          </label>
        );
      })}
    </div>
  );
}
