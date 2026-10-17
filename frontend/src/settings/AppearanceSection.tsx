import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { useT, type TranslationKey } from "../lib/i18n";
import { AppearancePreview } from "./AppearancePreview";
import { VolumeThresholdEditor } from "./VolumeThresholdEditor";
import type { DraftSetter, FieldErrors, SettingsConfig } from "./settingsTypes";

type AppearanceKey = keyof SettingsConfig["appearance"];

interface AppearanceOption {
  /** Raw persisted identifier; never translated. */
  value: string;
  labelKey: TranslationKey;
}

const APPEARANCE_FIELDS: Array<{
  key: AppearanceKey;
  labelKey: TranslationKey;
  options: AppearanceOption[];
}> = [
  {
    key: "theme",
    labelKey: "settings.theme",
    options: [
      { value: "System", labelKey: "theme.system" },
      { value: "Light", labelKey: "theme.light" },
      { value: "Dark", labelKey: "theme.dark" },
    ],
  },
  {
    key: "material",
    labelKey: "settings.material",
    options: [
      { value: "Auto", labelKey: "material.auto" },
      { value: "Translucent", labelKey: "material.translucent" },
      { value: "Opaque", labelKey: "material.opaque" },
    ],
  },
  {
    key: "motion",
    labelKey: "settings.motion",
    options: [
      { value: "Full", labelKey: "motion.full" },
      { value: "Reduced", labelKey: "motion.reduced" },
      { value: "Disabled", labelKey: "motion.disabled" },
    ],
  },
  {
    key: "accent",
    labelKey: "settings.accent",
    options: [
      { value: "System", labelKey: "accent.system" },
      { value: "Blue", labelKey: "accent.blue" },
      { value: "Green", labelKey: "accent.green" },
      { value: "Purple", labelKey: "accent.purple" },
      { value: "Orange", labelKey: "accent.orange" },
    ],
  },
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
  const tr = useT();
  const commit = (key: AppearanceKey, value: string) => {
    setDraft((d) => ({ ...d, appearance: { ...d.appearance, [key]: value } }));
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>{tr("settings.appearance")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {APPEARANCE_FIELDS.map(({ key, labelKey, options }) => (
          <label
            className="flex items-center justify-between gap-2 text-sm"
            key={key}
          >
            <span className="text-foreground/80">{tr(labelKey)}</span>
            <select
              aria-label={tr(labelKey)}
              className={SELECT_CLASS}
              value={draft.appearance[key]}
              onChange={(e) => commit(key, e.target.value)}
            >
              {options.map((option) => (
                <option key={option.value} value={option.value}>
                  {tr(option.labelKey)}
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
