import { Input } from "../components/ui/input";
import { FieldError } from "./FieldError";
import type { DraftSetter, FieldErrors, SettingsConfig } from "./settingsTypes";

type ThresholdKey = "green_up_to" | "blue_up_to" | "orange_up_to";

const THRESHOLD_FIELDS: Array<{ key: ThresholdKey; label: string }> = [
  { key: "green_up_to", label: "Green up to" },
  { key: "blue_up_to", label: "Blue up to" },
  { key: "orange_up_to", label: "Orange up to" },
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
  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-semibold">Volume thresholds</h3>
      {THRESHOLD_FIELDS.map(({ key, label }) => {
        const full = `color_thresholds.${key}`;
        return (
          <label className="flex flex-col gap-1 text-sm" key={key}>
            <span>{label}</span>
            <Input
              aria-label={label}
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
