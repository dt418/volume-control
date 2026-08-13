import { useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { invoke } from "../lib/ipc";
import type { DraftSetter, SettingsConfig } from "./settingsTypes";

/** Blacklist draft editor (legacy parity: subtitle, Add/Remove/Clear/Apply
 *  Recommended, and the "No blocked applications" empty-state copy). Edits
 *  stay in the draft; only Save commits the whole list atomically. */
export function BlacklistEditor({
  draft,
  setDraft,
}: {
  draft: SettingsConfig;
  setDraft: DraftSetter;
}) {
  const [entry, setEntry] = useState("");

  const add = () => {
    const value = entry.trim().toLowerCase();
    if (value && !draft.blacklist.includes(value)) {
      setDraft((d) => ({ ...d, blacklist: [...d.blacklist, value] }));
    }
    setEntry("");
  };

  const remove = (value: string) => {
    setDraft((d) => ({
      ...d,
      blacklist: d.blacklist.filter((v) => v !== value),
    }));
  };

  /** Merge the backend's recommended presets for the current modifier into
   *  the draft (dedup, preserve existing entries). */
  const mergeRecommended = async () => {
    const recommended = await invoke<string[]>("recommended_blacklist");
    setDraft((d) => ({
      ...d,
      blacklist: [
        ...new Set([...d.blacklist, ...recommended.map((v) => v.trim().toLowerCase())]),
      ],
    }));
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Blacklist</CardTitle>
        <p className="text-xs text-foreground/60">
          Block shortcuts while these apps have focus.
        </p>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex gap-2">
          <Input
            aria-label="Blocked application"
            value={entry}
            onChange={(e) => setEntry(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") add();
            }}
          />
          <button type="button" onClick={add}>
            Add
          </button>
        </div>
        {draft.blacklist.length === 0 ? (
          <div className="text-sm text-foreground/60">
            <p>No blocked applications</p>
            <p>VolumeControl will respond to shortcuts everywhere.</p>
          </div>
        ) : (
          <ul className="flex flex-col gap-2">
            {draft.blacklist.map((value) => (
              <li
                key={value}
                className="flex items-center justify-between text-sm"
              >
                <span>{value}</span>
                <button type="button" onClick={() => remove(value)}>
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => setDraft((d) => ({ ...d, blacklist: [] }))}
          >
            Clear
          </button>
          <button type="button" onClick={() => void mergeRecommended()}>
            Apply Recommended
          </button>
        </div>
      </CardContent>
    </Card>
  );
}
