import { useCallback, useEffect, useMemo, useState } from "react";

import { Card, CardContent } from "../components/ui/card";
import { Kbd } from "../components/ui/kbd";
import { applyAppearance, type AppearancePayload } from "../lib/appearance";
import { invoke, listen } from "../lib/ipc";
import { markSurfaceReady, surfaceErrorMessage } from "../lib/surface";
import type { HotkeyRegResult } from "../settings/KeyCard";
import { bindingsForModifier } from "../settings/modifierOptions";
import type { HotkeyBindings } from "../settings/settingsTypes";
import { ConflictCallout, conflictSentence, type ConflictCalloutData } from "./ConflictCallout";
import { HelpFooter } from "./HelpFooter";
import { HotkeyStatusBadge } from "./HotkeyStatusBadge";
import { baseActions, helpGroups, helpNote, type HelpShortcut } from "./shortcuts";
import { statusForAction } from "./status";

function SearchBox({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <input
      type="search"
      placeholder="Search shortcuts…"
      aria-label="Search shortcuts"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="mb-4 w-full rounded-md border border-foreground/20 bg-background/60 px-3 py-1.5 text-sm outline-none focus:border-accent"
    />
  );
}

function ShortcutCard({ item, status }: { item: HelpShortcut; status: ReturnType<typeof statusForAction> }) {
  return (
    <Card data-testid="help-card" className="p-0">
      <CardContent className="flex items-center justify-between gap-3 px-3 py-2">
        <span className="text-sm">{item.label}</span>
        <span className="flex items-center gap-2">
          <HotkeyStatusBadge kind={status} />
          <Kbd data-testid="help-combo">{item.combo}</Kbd>
        </span>
      </CardContent>
    </Card>
  );
}

interface BootstrapPayload {
  config: { modifier?: string; hotkeys?: HotkeyBindings };
  hotkey_status: HotkeyRegResult[];
  appearance: AppearancePayload;
}

/** Build the legacy conflict callout when any BASE action is in use by
 *  another app (legacy help.rs `conflict_callout` parity: base actions only,
 *  shift variants deduplicated). `None` when everything registered cleanly. */
function buildCallout(
  bindings: HotkeyBindings,
  status: HotkeyRegResult[],
): ConflictCalloutData | null {
  const conflicted = baseActions(bindings).filter(
    (item) => statusForAction(status, item.action) === "in-use",
  );
  if (conflicted.length === 0) return null;
  return {
    combos: conflicted.map((item) => item.combo),
    tail: conflicted.length === 1 ? "is used by another app." : "are used by another app.",
  };
}

export function HelpSurface() {
  const [modifier, setModifier] = useState("CtrlAlt");
  const [bindings, setBindings] = useState<HotkeyBindings>(() => bindingsForModifier("CtrlAlt"));
  const [hotkeyStatus, setHotkeyStatus] = useState<HotkeyRegResult[]>([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    invoke<BootstrapPayload>("get_bootstrap")
      .then((bootstrap) => {
        if (alive) {
          setError(null);
          applyAppearance(bootstrap.appearance as AppearancePayload);
          const nextModifier = bootstrap.config?.modifier ?? "CtrlAlt";
          setModifier(nextModifier);
          setBindings(bootstrap.config?.hotkeys ?? bindingsForModifier(nextModifier));
          setHotkeyStatus(bootstrap.hotkey_status ?? []);
        }
      })
      .catch((reason) => {
        if (alive) setError(surfaceErrorMessage(reason));
      })
      .finally(() => {
        if (alive) void markSurfaceReady();
      });

    // Live registration status: a modifier change in Settings or an external
    // conflict appears here without a reload.
    let unlisten: (() => void) | undefined;
    void listen<HotkeyRegResult[]>("state://hotkeys", (status) => {
      if (!alive) return;
      setHotkeyStatus(status);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  const groups = useMemo(() => helpGroups(bindings), [bindings]);
  const note = helpNote(modifier);
  const callout = useMemo(
    () => buildCallout(bindings, hotkeyStatus),
    [bindings, hotkeyStatus],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return groups;
    return groups
      .map((group) => ({
        ...group,
        items: group.items.filter(
          (item) =>
            item.label.toLowerCase().includes(q) || item.combo.toLowerCase().includes(q),
        ),
      }))
      .filter((group) => group.items.length > 0);
  }, [groups, query]);

  const openSettings = useCallback(() => {
    void invoke("open_surface", { surface: "window-settings" });
  }, []);

  const close = useCallback(() => {
    void invoke("close_surface", { surface: "window-help" });
  }, []);

  const onKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    },
    [close],
  );

  useEffect(() => {
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onKeyDown]);

  return (
    <main
      data-surface="help"
      className="surface-shell h-dvh min-h-0 overflow-hidden flex flex-col bg-background"
    >
      {/* Header band (legacy parity): 3px accent bar, title, subtitle, close × */}
      <header data-testid="surface-header" className="relative flex-shrink-0 border-b border-foreground/10">
        <div aria-hidden="true" className="absolute inset-x-0 top-0 h-[3px] bg-accent" />
        <div className="flex items-start justify-between px-4 pb-2 pt-6">
          <div>
            <h1 className="text-lg font-semibold">VolumeControl</h1>
            <p className="text-sm text-foreground/60">Keyboard shortcuts</p>
          </div>
          <button
            type="button"
            aria-label="Close"
            onClick={close}
            className="rounded-md px-2 py-1 text-foreground/60 hover:bg-foreground/5 hover:text-foreground"
          >
            ×
          </button>
        </div>
      </header>

      <div data-testid="surface-content" className="surface-scroll min-h-0 flex-1 overflow-y-auto">
        <div className="flex flex-col gap-4 p-4">
          {error ? (
            <div role="alert" className="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
              Help unavailable: {error}
            </div>
          ) : (
            <>
              {note && <p className="text-sm text-foreground/60">{note}</p>}
              <SearchBox value={query} onChange={setQuery} />

              {callout && <ConflictCallout data={callout} onOpenSettings={openSettings} />}

              {filtered.map((group) => (
                <section key={group.id} className="flex flex-col gap-2">
                  <h2 className="text-sm font-medium text-foreground/70">{group.title}</h2>
                  <div className="grid grid-cols-1 gap-2">
                    {group.items.map((item) => (
                      <ShortcutCard
                        key={item.action}
                        item={item}
                        status={statusForAction(hotkeyStatus, item.action)}
                      />
                    ))}
                  </div>
                </section>
              ))}
              {filtered.length === 0 && (
                <p className="text-sm text-foreground/50">No shortcuts match “{query}”.</p>
              )}
            </>
          )}
        </div>
      </div>

      <HelpFooter
        onEditConfig={() => void invoke("open_config_location")}
        onSettings={openSettings}
        onClose={close}
      />
    </main>
  );
}
