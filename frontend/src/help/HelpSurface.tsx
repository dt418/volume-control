import { useCallback, useEffect, useMemo, useState } from "react";

import { Card, CardContent } from "../components/ui/card";
import { Kbd } from "../components/ui/kbd";
import { applyAppearance, type AppearancePayload } from "../lib/appearance";
import { invoke } from "../lib/ipc";
import { helpGroups, helpNote, HelpShortcut } from "./shortcuts";

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

function ShortcutCard({ item }: { item: HelpShortcut }) {
  return (
    <Card data-testid="help-card" className="p-0">
      <CardContent className="flex items-center justify-between gap-3 px-3 py-2">
        <span className="text-sm">{item.label}</span>
        <Kbd data-testid="help-combo">{item.combo}</Kbd>
      </CardContent>
    </Card>
  );
}

export function HelpSurface() {
  const [modifier, setModifier] = useState("CtrlAlt");
  const [query, setQuery] = useState("");

  useEffect(() => {
    let alive = true;
    invoke<{ config: { modifier?: string }; appearance: AppearancePayload }>("get_bootstrap")
      .then((bootstrap) => {
        if (alive) {
          applyAppearance(bootstrap.appearance as AppearancePayload);
          setModifier(bootstrap.config?.modifier ?? "CtrlAlt");
        }
      })
      .catch(() => {
        // Fail-soft: keep the default modifier so the grid still renders.
      });
    return () => {
      alive = false;
    };
  }, []);

  const groups = useMemo(() => helpGroups(modifier), [modifier]);
  const note = helpNote(modifier);

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

  const onKeyDown = useCallback((event: KeyboardEvent) => {
    if (event.key === "Escape") {
      void invoke("close_surface", { surface: "window-help" });
    }
  }, []);

  useEffect(() => {
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onKeyDown]);

  return (
    <main className="min-h-screen bg-background p-4">
      <h1 className="mb-1 text-lg font-semibold">Shortcuts</h1>
      <p className="mb-3 text-sm text-foreground/60">
        Global keyboard shortcuts{note ? ` — ${note}` : ""}
      </p>
      <SearchBox value={query} onChange={setQuery} />
      {filtered.map((group) => (
        <section key={group.id} className="mb-4">
          <h2 className="mb-2 text-sm font-medium text-foreground/70">{group.title}</h2>
          <div className="grid grid-cols-1 gap-2">
            {group.items.map((item) => (
              <ShortcutCard key={item.action} item={item} />
            ))}
          </div>
        </section>
      ))}
      {filtered.length === 0 && (
        <p className="text-sm text-foreground/50">No shortcuts match “{query}”.</p>
      )}
    </main>
  );
}
