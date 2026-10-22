import { useEffect, useId, useMemo, useState } from "react";

import { Input } from "../components/ui/input";
import { AppIcon } from "./AppIcon";
import { appLabel, type AppLabelStyle } from "../lib/appLabel";
import { useT } from "../lib/i18n";
import { invoke } from "../lib/ipc";

export interface RunningApp {
  process: string;
  title: string;
  display_name?: string;
  icon?: string;
}

/** Collapsible picker for processes that currently own a visible window.
 *  Checking a row edits the draft blacklist immediately; Save still owns the
 *  backend commit, so closing the picker never persists anything. */
export function RunningAppPicker({
  blacklist,
  onToggle,
  nameStyle = "friendly",
}: {
  blacklist: string[];
  onToggle: (process: string) => void;
  nameStyle?: AppLabelStyle;
}) {
  const tr = useT();
  const [open, setOpen] = useState(false);
  const [apps, setApps] = useState<RunningApp[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [refreshToken, setRefreshToken] = useState(0);
  const panelId = useId();
  const searchId = useId();

  useEffect(() => {
    if (!open) return;
    let disposed = false;
    setLoading(true);
    setError(null);
    void invoke<RunningApp[]>("running_apps")
      .then((value) => {
        if (!disposed) setApps(value);
      })
      .catch((reason) => {
        if (!disposed) {
          setApps([]);
          setError(String(reason));
        }
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [open, refreshToken]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return apps;
    return apps.filter(
      (app) =>
        app.process.toLowerCase().includes(needle) ||
        app.display_name?.toLowerCase().includes(needle) ||
        app.title.toLowerCase().includes(needle),
    );
  }, [apps, query]);

  const selected = apps.filter((app) => blacklist.includes(app.process)).length;

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        title={tr("blacklist.chooseAppsHint")}
        onClick={() => setOpen((value) => !value)}
        className="self-start rounded-md border border-foreground/20 px-3 py-1.5 text-sm transition-colors hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
      >
        {tr("blacklist.chooseApps")}
      </button>

      {open && (
        <section
          id={panelId}
          aria-label={tr("blacklist.appPicker")}
          className="flex flex-col gap-2 rounded-lg border border-foreground/15 bg-foreground/[0.03] p-3"
        >
          <div className="flex gap-2">
            <label className="sr-only" htmlFor={searchId}>
              {tr("blacklist.searchApps")}
            </label>
            <Input
              id={searchId}
              autoFocus
              value={query}
              placeholder={tr("blacklist.searchApps")}
              title={tr("blacklist.searchApps")}
              onChange={(event) => setQuery(event.target.value)}
            />
            <button
              type="button"
              onClick={() => setRefreshToken((value) => value + 1)}
              title={tr("blacklist.refreshAppsHint")}
              disabled={loading}
              className="shrink-0 rounded-md border border-foreground/20 px-3 py-1 text-sm disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            >
              {tr("blacklist.refresh")}
            </button>
          </div>

          {loading ? (
            <p className="px-1 py-3 text-sm text-foreground/60" role="status">
              {tr("blacklist.loadingApps")}
            </p>
          ) : error ? (
            <p role="alert" className="px-1 py-2 text-sm text-amber-600 dark:text-amber-300">
              {tr("blacklist.appsLoadFailed")}
            </p>
          ) : filtered.length === 0 ? (
            <p className="px-1 py-3 text-sm text-foreground/60">
              {apps.length === 0
                ? tr("blacklist.noRunningApps")
                : tr("blacklist.noAppMatches")}
            </p>
          ) : (
            <ul className="flex max-h-56 flex-col gap-1 overflow-y-auto surface-scroll">
              {filtered.map((app) => {
                const checked = blacklist.includes(app.process);
                const label = appLabel(nameStyle, app.display_name, app.process);
                return (
                  <li key={app.process}>
                    <label className="flex cursor-pointer items-center gap-2 rounded-md px-1.5 py-1.5 text-sm hover:bg-foreground/5">
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => onToggle(app.process)}
                        aria-label={`${checked ? tr("blacklist.unblockApp") : tr("blacklist.blockApp")} ${label}`}
                        className="h-4 w-4 shrink-0 accent-current focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
                      />
                      <AppIcon src={app.icon} label={label} />
                      <span className="min-w-0 flex-1 truncate" title={app.title || label}>
                        {label}
                      </span>
                    </label>
                  </li>
                );
              })}
            </ul>
          )}

          <div className="flex items-center justify-between gap-2 text-xs text-foreground/60">
            <span aria-live="polite">
              {tr("blacklist.selected")}: {selected}
            </span>
            <button
              type="button"
              onClick={() => setOpen(false)}
              title={tr("blacklist.doneHint")}
              className="rounded-md border border-foreground/20 px-3 py-1 text-foreground/80 hover:bg-foreground/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            >
              {tr("blacklist.done")}
            </button>
          </div>
        </section>
      )}
    </div>
  );
}
