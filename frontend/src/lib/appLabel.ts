export type AppLabelStyle = "friendly" | "friendly_and_process" | "process";

const STYLES: readonly AppLabelStyle[] = ["friendly", "friendly_and_process", "process"];

/** Hydrate a persisted style, defaulting unknown/missing values to friendly. */
export function asAppLabelStyle(value: string | undefined | null): AppLabelStyle {
  return STYLES.includes(value as AppLabelStyle) ? (value as AppLabelStyle) : "friendly";
}

/** Render one app label for the mixer, picker, and blacklist rows. */
export function appLabel(
  style: AppLabelStyle,
  displayName: string | undefined | null,
  process: string,
): string {
  const friendly = displayName?.trim() || process;
  switch (style) {
    case "process":
      return process;
    case "friendly_and_process":
      return friendly.toLowerCase() === process.toLowerCase()
        ? process
        : `${friendly} (${process})`;
    default:
      return friendly;
  }
}
