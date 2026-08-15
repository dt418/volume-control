import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { AppearancePreview } from "./AppearancePreview";
import { VolumeThresholdEditor } from "./VolumeThresholdEditor";
import type { DraftSetter, FieldErrors, SettingsConfig } from "./settingsTypes";

type AppearanceKey = keyof SettingsConfig["appearance"];

const APPEARANCE_FIELDS: Array<{ key: AppearanceKey; options: string[] }> = [
  { key: "theme", options: ["System", "Light", "Dark"] },
  { key: "material", options: ["Auto", "Translucent", "Opaque"] },
  { key: "motion", options: ["Full", "Reduced", "Disabled"] },
  { key: "accent", options: ["System", "Blue", "Green", "Purple", "Orange"] },
];

const SELECT_CLASS =
  "h-8 rounded-md border border-foreground/20 bg-background px-2 text-sm shadow-sm " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent";

/** Appearance section (legacy parity): theme/material/motion/accent selects,
 *  a draft-driven mini Signal Rail preview, and the volume threshold editor. */
export function AppearanceSection({
  draft,
  setDraft,
  errors,
}: {
  draft: SettingsConfig;
  setDraft: DraftSetter;
  errors: FieldErrors;
}) {
  const commit = (key: AppearanceKey, value: string) => {
    setDraft((d) => ({ ...d, appearance: { ...d.appearance, [key]: value } }));
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Appearance</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {APPEARANCE_FIELDS.map(({ key, options }) => (
          <label
            className="flex items-center justify-between gap-2 text-sm"
            key={key}
          >
            <span className="text-foreground/80">
              {key[0].toUpperCase() + key.slice(1)}
            </span>
            <select
              aria-label={key[0].toUpperCase() + key.slice(1)}
              className={SELECT_CLASS}
              value={draft.appearance[key]}
              onChange={(e) => commit(key, e.target.value)}
            >
              {options.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
        ))}
        <AppearancePreview draft={draft} />
        <VolumeThresholdEditor draft={draft} setDraft={setDraft} errors={errors} />
      </CardContent>
    </Card>
  );
}
