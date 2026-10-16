import { useT, type TranslationKey } from "../lib/i18n";

export const SETTINGS_SECTIONS = [
  "General",
  "Hotkeys",
  "Appearance",
  "Blacklist",
  "Feedback",
  "Storage",
] as const;

export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];

/** Section identity stays canonical English; only the label is translated. */
const SECTION_LABELS: Record<SettingsSection, TranslationKey> = {
  General: "settings.general",
  Hotkeys: "settings.hotkeys",
  Appearance: "settings.appearance",
  Blacklist: "settings.blacklist",
  Feedback: "settings.feedback",
  Storage: "settings.storage",
};

/** Six-section navigation (legacy parity): vertical rail on wide screens,
 *  horizontal strip below 760px. One content pane swaps per selection. */
export function SectionNav({
  active,
  onSelect,
}: {
  active: SettingsSection;
  onSelect: (section: SettingsSection) => void;
}) {
  const tr = useT();
  return (
    <nav
      aria-label="Settings sections"
      className="flex flex-row gap-1 overflow-x-auto min-[760px]:flex-col"
    >
      {SETTINGS_SECTIONS.map((section) => (
        <button
          type="button"
          key={section}
          aria-pressed={active === section}
          onClick={() => onSelect(section)}
          className={`shrink-0 rounded-md px-3 py-2 text-left text-sm ${
            active === section
              ? "bg-accent/15 text-accent"
              : "text-foreground/70 hover:bg-foreground/5"
          }`}
        >
          {tr(SECTION_LABELS[section])}
        </button>
      ))}
    </nav>
  );
}
