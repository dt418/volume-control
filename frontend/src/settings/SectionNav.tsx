export const SETTINGS_SECTIONS = [
  "General",
  "Hotkeys",
  "Appearance",
  "Blacklist",
  "Feedback",
  "Storage",
] as const;

export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];

/** Six-section navigation (legacy parity): vertical rail on wide screens,
 *  horizontal strip below 760px. One content pane swaps per selection. */
export function SectionNav({
  active,
  onSelect,
}: {
  active: SettingsSection;
  onSelect: (section: SettingsSection) => void;
}) {
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
          {section}
        </button>
      ))}
    </nav>
  );
}
