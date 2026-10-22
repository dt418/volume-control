import { useEffect, useId, useState } from "react";
import { Trash2 } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { appLabel, asAppLabelStyle } from "../lib/appLabel";
import { useT } from "../lib/i18n";
import { invoke } from "../lib/ipc";
import { AppIcon } from "./AppIcon";
import { RunningAppPicker, type RunningApp } from "./RunningAppPicker";
import type { DraftSetter, SettingsConfig } from "./settingsTypes";

const normalize = (value: string) => value.trim().toLowerCase();

/** Blacklist draft editor. Entries can be typed manually or selected from the
 *  running-app picker; edits stay in the draft until Save commits the list. */
export function BlacklistEditor({
  draft,
  setDraft,
  saved = [],
}: {
  draft: SettingsConfig;
  setDraft: DraftSetter;
  saved?: string[];
}) {
  const [entry, setEntry] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const [runningApps, setRunningApps] = useState<Record<string, RunningApp>>({});
  const subtitleId = useId();
  const tr = useT();
  const nameStyle = asAppLabelStyle(draft.app_label_style);
  const blacklistKey = draft.blacklist.join("\u0000");

  // Blacklist rows are stored as process names. Resolve icons and friendly
  // product names for both running and installed (not currently running)
  // applications; unresolved entries keep the letter-tile fallback.
  useEffect(() => {
    let disposed = false;
    const processes = blacklistKey ? blacklistKey.split("\u0000") : [];
    void invoke<RunningApp[]>("resolve_apps", { processes })
      .then((apps) => {
        if (!disposed) {
          setRunningApps(Object.fromEntries(apps.map((app) => [app.process, app])));
        }
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, [blacklistKey]);

  const add = () => {
    const value = normalize(entry);
    if (value) {
      setDraft((d) => ({
        ...d,
        blacklist: d.blacklist.includes(value) ? d.blacklist : [...d.blacklist, value],
      }));
    }
    setEntry("");
  };

  const remove = (value: string) => {
    setDraft((d) => ({
      ...d,
      blacklist: d.blacklist.filter((v) => v !== value),
    }));
  };

  const toggle = (value: string) => {
    const normalized = normalize(value);
    if (!normalized) return;
    setDraft((d) => ({
      ...d,
      blacklist: d.blacklist.includes(normalized)
        ? d.blacklist.filter((v) => v !== normalized)
        : [...d.blacklist, normalized],
    }));
  };

  const recommended = async (mode: "merge" | "replace") => {
    setActionError(null);
    try {
      const values = await invoke<string[]>("recommended_blacklist");
      const normalized = [...new Set(values.map(normalize).filter(Boolean))];
      setDraft((d) => ({
        ...d,
        blacklist:
          mode === "merge" ? [...new Set([...d.blacklist, ...normalized])] : normalized,
      }));
    } catch (reason) {
      setActionError(String(reason));
    }
  };

  const draftChanged =
    draft.blacklist.length !== saved.length ||
    draft.blacklist.some((value, index) => value !== saved[index]);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{tr("settings.blacklist")}</CardTitle>
        <p id={subtitleId} className="text-xs text-foreground/60">
          {tr("blacklist.subtitle")}
        </p>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex gap-2">
          <Input
            aria-label={tr("blacklist.blockedApplication")}
            aria-describedby={subtitleId}
            title={tr("blacklist.addHint")}
            placeholder={tr("blacklist.manualPlaceholder")}
            value={entry}
            onChange={(e) => setEntry(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") add();
            }}
          />
          <button
            type="button"
            onClick={add}
            title={tr("blacklist.addHint")}
            className="shrink-0 rounded-md border border-foreground/20 px-3 py-1 text-sm hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            {tr("blacklist.add")}
          </button>
        </div>

        <RunningAppPicker
          blacklist={draft.blacklist}
          onToggle={toggle}
          nameStyle={nameStyle}
        />

        {actionError && (
          <p role="alert" className="text-xs text-amber-600 dark:text-amber-300">
            {actionError}
          </p>
        )}

        {draft.blacklist.length === 0 ? (
          <div className="rounded-md border border-dashed border-foreground/15 px-3 py-4 text-sm text-foreground/60">
            <p>{tr("blacklist.empty")}</p>
            <p className="text-xs">{tr("blacklist.emptyHint")}</p>
          </div>
        ) : (
          <>
            <div className="flex items-center justify-between text-xs text-foreground/60">
              <span>{tr("settings.blacklist")}</span>
              <span aria-live="polite">{draft.blacklist.length}</span>
            </div>
            <ul className="divide-y divide-foreground/10 rounded-md border border-foreground/15">
              {draft.blacklist.map((value) => {
                const app = runningApps[value];
                const label = appLabel(nameStyle, app?.display_name, value);
                return (
                  <li
                    key={value}
                    className="flex items-center justify-between gap-2 px-2.5 py-1.5"
                  >
                    <div className="flex min-w-0 flex-1 items-center gap-2">
                      <AppIcon src={app?.icon} label={label} />
                      <div className="min-w-0">
                        <div
                          className="truncate text-xs font-medium"
                          title={app?.title || label}
                        >
                          {label}
                        </div>
                      </div>
                    </div>
                    <button
                      type="button"
                      aria-label={`${tr("blacklist.remove")} ${label}`}
                      title={`${tr("blacklist.remove")} ${label}`}
                      onClick={() => remove(value)}
                      className="rounded p-1 text-foreground/50 hover:bg-foreground/5 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
                    >
                      <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
                    </button>
                  </li>
                );
              })}
            </ul>
          </>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void recommended("merge")}
            title={tr("blacklist.applyRecommendedHint")}
            className="rounded-md border border-foreground/20 px-3 py-1 text-sm hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            {tr("blacklist.applyRecommended")}
          </button>
          <button
            type="button"
            onClick={() => void recommended("replace")}
            title={tr("blacklist.restoreDefaultsHint")}
            className="rounded-md border border-foreground/20 px-3 py-1 text-sm hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            {tr("blacklist.restoreDefaults")}
          </button>
          <button
            type="button"
            disabled={!draftChanged}
            onClick={() => setDraft((d) => ({ ...d, blacklist: [...saved] }))}
            title={tr("blacklist.reloadSavedHint")}
            className="rounded-md border border-foreground/20 px-3 py-1 text-sm hover:bg-foreground/5 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            {tr("blacklist.reloadSaved")}
          </button>
          <button
            type="button"
            disabled={draft.blacklist.length === 0}
            onClick={() => setDraft((d) => ({ ...d, blacklist: [] }))}
            title={tr("blacklist.clearHint")}
            className="rounded-md border border-foreground/20 px-3 py-1 text-sm text-foreground/70 hover:bg-foreground/5 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            {tr("blacklist.clear")}
          </button>
          <span className="text-xs text-foreground/50">{tr("blacklist.unsavedHint")}</span>
        </div>
        <p className="text-xs text-foreground/50">{tr("blacklist.actionsHint")}</p>
      </CardContent>
    </Card>
  );
}
