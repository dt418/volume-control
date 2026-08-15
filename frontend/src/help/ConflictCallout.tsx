import { Card, CardContent } from "../components/ui/card";
import { Kbd } from "../components/ui/kbd";

export interface ConflictCalloutData {
  /** Conflicted base-action combos, one chip list per row (row order,
   *  shift variants deduplicated — legacy help.rs ConflictCallout). */
  combos: string[];
  /** Sentence tail: "is used by another app." / "are used by another app." */
  tail: string;
}

/** The conflicted-combo sentence: "<c1> and <c2> are used by another app."
 *  (connector ", " between, " and " before the last — legacy
 *  `explanation_atoms`). */
export function conflictSentence(combos: string[], tail: string): string {
  if (combos.length === 0) return "";
  let sentence = combos[0];
  for (let i = 1; i < combos.length; i += 1) {
    sentence += i === combos.length - 1 ? " and " : ", ";
    sentence += combos[i];
  }
  return `${sentence} ${tail}`;
}

/** Legacy conflict callout card: warning marker, conflicted combo keycaps,
 *  the sentence, and the next action ("Change the modifier in Settings.").
 *  `onOpenSettings` is the CTA — it opens the Settings surface. */
export function ConflictCallout({
  data,
  onOpenSettings,
}: {
  data: ConflictCalloutData;
  onOpenSettings: () => void;
}) {
  return (
    <Card
      data-testid="conflict-callout"
      className="border border-amber-500/40 bg-amber-500/5"
    >
      <CardContent className="flex flex-col gap-2 px-3 py-2">
        <div className="flex items-center gap-2">
          <span aria-hidden="true" className="text-amber-500">
            ⚠
          </span>
          <h2 className="text-sm font-semibold">Shortcut conflict</h2>
        </div>
        <p className="text-sm text-foreground/80">
          {conflictSentence(data.combos, data.tail)}{" "}
          <button
            type="button"
            onClick={onOpenSettings}
            className="text-accent underline underline-offset-2 hover:text-accent/80"
          >
            Change the modifier in Settings.
          </button>
        </p>
        <div className="flex flex-wrap gap-1">
          {data.combos.map((combo) => (
            <Kbd key={combo}>{combo}</Kbd>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
