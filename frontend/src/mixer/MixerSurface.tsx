import { useCallback, useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { Search } from "lucide-react";

import { invoke, listen } from "../lib/ipc";
import { surfaceErrorMessage } from "../lib/surface";
import { SessionRow } from "./SessionRow";
import { SystemOutputRow } from "./SystemOutputRow";
import { useSessions } from "./sessionStore";

/**
 * Mixer webview surface (spec §5.1): per-app sliders with quick mute, search
 * filter, active-first sort, live updates via state:// events, Esc-to-close
 * and the fail-soft stale-session handling of spec §9.6.
 */
export function MixerSurface() {
  const {
    sessions,
    volumePct,
    muted,
    thresholds,
    sessionsSupported,
    notice,
    error,
    retry,
    setNotice,
    removeSession,
    updateSession,
  } = useSessions();
  const [query, setQuery] = useState("");

  // Esc fallback for closing the frameless mixer window (spec §9.3); the Rust
  // side additionally closes it on Focused(false) for outside clicks.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        void invoke<void>("close_surface", { surface: "window-mixer" }).catch(() => {});
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // The Rust WindowManager emits `close_mixer_request` when the window loses
  // focus (outside click / Alt+Tab) — WindowEvent::Focused(false) is reliable
  // on Win32 and Wayland, JS blur is not (spec §9.3). Close through the same
  // command so the surface leaves the `active` set and can be reopened.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<void>("close_mixer_request", () => {
      void invoke<void>("close_surface", { surface: "window-mixer" }).catch(() => {});
    }).then((u) => {
      unlisten = u;
    });
    return () => unlisten?.();
  }, []);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((s) => s.name.toLowerCase().includes(q));
  }, [sessions, query]);

  const handleMute = useCallback(
    (id: string) => {
      const row = sessions.find((s) => s.id === id);
      const nextMuted = !row?.muted;
      updateSession(id, { muted: nextMuted }); // optimistic; backend does not re-emit on success
      void invoke<void>("mute_session", { id }).catch(() => {
        updateSession(id, { muted: !nextMuted }); // revert on failure
      });
    },
    [sessions, updateSession],
  );

  const handleVolumeError = useCallback(
    (id: string, error: string) => {
      // Stale session: drop the row and inform the user instead of erroring (spec §9.6).
      removeSession(id);
      setNotice(`${error} — session removed (no longer playing audio)`);
    },
    [removeSession, setNotice],
  );

  return (
    <main data-surface="mixer" className="mixer-surface surface-shell h-dvh min-h-0 overflow-hidden glass-surface flex flex-col gap-1 p-2">
      <header data-testid="surface-header" className="flex flex-shrink-0 flex-col gap-2">
        <h1 className="text-sm font-semibold">Volume Mixer</h1>
        <div className="text-xs text-foreground/80">
          {muted ? <span className="font-medium">Muted</span> : `Volume ${volumePct}%`}
        </div>

      <div className="relative">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-foreground/65" />
        <input
          type="text"
          placeholder="Search apps…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          className="w-full rounded-md border border-foreground/20 bg-background/88 py-1.5 pl-8 pr-3 text-xs outline-none placeholder:text-foreground/60 focus:border-accent focus:ring-2 focus:ring-accent/35"
        />
      </div>

      <SystemOutputRow value={volumePct} muted={muted} thresholds={thresholds} />
      </header>

      {notice && (
        <div
          role="status"
          className="rounded-md bg-amber-500/15 px-3 py-1.5 text-xs text-amber-600 dark:text-amber-400"
        >
          {notice}
        </div>
      )}

      <div data-testid="surface-content" className="surface-content surface-scroll min-h-0 flex-1 space-y-2 overflow-y-auto">
        {error ? (
          <div role="alert" aria-live="assertive" className="flex items-start justify-between gap-3 rounded-md border border-amber-500/45 bg-amber-500/12 p-3 text-xs text-foreground">
            <div className="min-w-0">
              <p className="font-semibold">Mixer connection needs attention</p>
              <p className="mt-1 break-words text-foreground/75">{surfaceErrorMessage(error)}</p>
            </div>
            <button
              type="button"
              onClick={retry}
              className="shrink-0 rounded-md border border-foreground/25 bg-background/80 px-2 py-1 font-medium text-foreground transition-colors hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            >
              Retry
            </button>
          </div>
        ) : !sessionsSupported ? (
          <p className="py-8 text-center text-xs text-foreground/70">
            Per-app mixing is Windows-only
          </p>
        ) : filtered.length === 0 ? (
          <p className="py-8 text-center text-xs text-foreground/70">
            {sessions.length === 0 ? "No audio sessions" : "No matching apps"}
          </p>
        ) : (
          filtered.map((session, index) => (
            <motion.div
              key={`${session.id}-${session.name}-${index}`}
              layout
              transition={{ duration: 0.15 }}
            >
              <SessionRow
                session={session}
                onMute={handleMute}
                onVolumeError={handleVolumeError}
              />
            </motion.div>
          ))
        )}
      </div>

      <footer data-testid="surface-footer" className="flex flex-shrink-0 items-center justify-between border-t border-foreground/20 pt-1 text-[11px] text-foreground/70">
        <span>{sessionsSupported ? `${filtered.length} app sessions` : "System output"}</span>
        {error ? <span className="font-medium text-amber-700 dark:text-amber-300">Retry connection</span> : <span>Esc to close</span>}
      </footer>
    </main>
  );
}
