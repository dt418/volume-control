import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { AutoStartControl } from "./AutoStartControl";
import { FieldError } from "./FieldError";
import type { DraftSetter, FieldErrors, SettingsConfig } from "./settingsTypes";

interface GeneralField {
  key: "volume_step" | "volume_step_large" | "overlay_duration_ms";
  label: string;
  help: string;
  min: number;
  max: number;
}

const GENERAL_FIELDS: GeneralField[] = [
  {
    key: "volume_step",
    label: "Volume step",
    help: "Small change applied by ↑ / ↓.",
    min: 1,
    max: 50,
  },
  {
    key: "volume_step_large",
    label: "Large volume step",
    help: "Shift + ↑ / ↓ applies the large change.",
    min: 1,
    max: 50,
  },
  {
    key: "overlay_duration_ms",
    label: "Overlay duration",
    help: "How long the volume overlay stays visible.",
    min: 200,
    max: 10000,
  },
];

/** General section (legacy parity): step sizes + overlay duration with
 *  inline field errors. */
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
  return (
    <Card>
      <CardHeader>
        <CardTitle>General</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {GENERAL_FIELDS.map(({ key, label, help, min, max }) => (
          <label className="flex flex-col gap-1 text-sm" key={key}>
            <span>{label}</span>
            <span className="text-xs text-foreground/60">{help}</span>
            <Input
              aria-label={label}
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
  );
}
