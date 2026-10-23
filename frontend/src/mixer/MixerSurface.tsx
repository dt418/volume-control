import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { motion } from "framer-motion";
import { CircleHelp, Search, Settings, X } from "lucide-react";

import { SurfaceCloseButton } from "../components/ui/surface-close-button";
import { appLabel } from "../lib/appLabel";
import { invoke, listen } from "../lib/ipc";
import { translatorFor, LanguageProvider } from "../lib/i18n";
import { surfaceErrorMessage } from "../lib/surface";
import type { UpdateStatePayload } from "../lib/update";
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
    language,
    appLabelStyle,
    sessionsSupported,
    notice,
    error,
    retry,
    setNotice,
    removeSession,
    updateSession,
  } = useSessions();
  const [query, setQuery] = useState("");
  const [updateState, setUpdateState] = useState<UpdateStatePayload | null>(null);
  const [dismissedVersion, setDismissedVersion] = useState<string | null>(null);
  const lastRevision = useRef<number>(-1);
  const tr = translatorFor(language);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<UpdateStatePayload>("state://update", (payload) => {
      if (!disposed && payload.revision > lastRevision.current) {
        lastRevision.current = payload.revision;
        setUpdateState(payload);
      }
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    void invoke<UpdateStatePayload>("get_update_state")
      .then((payload) => {
        if (!disposed && payload.revision > lastRevision.current) {
          lastRevision.current = payload.revision;
          setUpdateState(payload);
        }
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const close = useCallback(() => {
    void invoke<void>("close_surface", { surface: "window-mixer" }).catch(() => {});
  }, []);

  const openSurface = useCallback((surface: "window-settings" | "window-help") => {
    void invoke<void>("open_surface", { surface }).catch(() => {});
  }, []);

  // Esc fallback for closing the frameless mixer window (spec §9.3); the Rust
  // side additionally closes it on Focused(false) for outside clicks.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [close]);

  // The Rust WindowManager emits `close_mixer_request` when the window loses
  // focus (outside click / Alt+Tab) — WindowEvent::Focused(false) is reliable
  // on Win32 and Wayland, JS blur is not (spec §9.3). Close through the same
  // command so the surface leaves the `active` set and can be reopened.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<void>("close_mixer_request", () => {
      // A close event can be queued while the window is losing focus and be
      // delivered after a second launch has reopened it. Only close if the
      // window is still unfocused.
      if (document.hasFocus()) return;
      void invoke<void>("close_surface", { surface: "window-mixer" }).catch(() => {});
    }).then((u) => {
      unlisten = u;
    });
    return () => unlisten?.();
  }, [close]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((s) => {
      const process = s.process ?? s.name;
      const label = s.system
        ? tr("mixer.systemSounds")
        : appLabel(appLabelStyle, s.display_name, process);
      return (
        label.toLowerCase().includes(q) || process.toLowerCase().includes(q)
      );
    });
  }, [appLabelStyle, sessions, query, tr]);

  const handleMute = useCallback(
    (id: string) => {
      const row = sessions.find((s) => s.id === id);
      const nextMuted = !row?.muted;
      updateSession(id, { muted: nextMuted }); // optimistic; backend does not re-emit on success
      void invoke<void>("mute_session", { id }).catch((reason) => {
        updateSession(id, { muted: !nextMuted }); // revert on failure
        setNotice(`${tr("mixer.muteFailed")}: ${surfaceErrorMessage(reason)}`);
      });
    },
    [sessions, updateSession, setNotice, tr],
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
    <LanguageProvider language={language}>
    <main data-surface="mixer" className="mixer-surface surface-shell h-dvh min-h-0 overflow-hidden glass-surface flex flex-col gap-1 p-2">
      <header data-testid="surface-header" className="flex flex-shrink-0 flex-col gap-2">
        <div className="flex items-center justify-between gap-2">
          <div className="flex min-w-0 items-baseline gap-2">
            <h1 className="truncate text-sm font-semibold">{tr("mixer.title")}</h1>
            <span className="shrink-0 text-xs text-foreground/80">
              {muted ? (
                <span className="font-medium">{tr("common.muted")}</span>
              ) : (
                `${tr("common.volume")} ${volumePct}%`
              )}
            </span>
          </div>
          <div className="flex shrink-0 items-center gap-0.5">
            <button
              type="button"
              aria-label={tr("settings.title")}
              title={tr("settings.title")}
              onClick={() => openSurface("window-settings")}
              className="inline-flex h-8 w-8 items-center justify-center rounded-md text-foreground/65 transition-colors hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent active:bg-foreground/15"
            >
              <Settings aria-hidden="true" className="h-4 w-4" />
            </button>
            <button
              type="button"
              aria-label={tr("help.title")}
              title={tr("help.title")}
              onClick={() => openSurface("window-help")}
              className="inline-flex h-8 w-8 items-center justify-center rounded-md text-foreground/65 transition-colors hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent active:bg-foreground/15"
            >
              <CircleHelp aria-hidden="true" className="h-4 w-4" />
            </button>
            <SurfaceCloseButton
              label={tr("mixer.close")}
              title={`${tr("mixer.close")} (Esc)`}
              onClick={close}
            />
          </div>
        </div>

      <div className="relative">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-foreground/65" />
        <input
          type="text"
          placeholder={tr("mixer.searchPlaceholder")}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          className="w-full rounded-md border border-foreground/20 bg-background/88 py-1.5 pl-8 pr-8 text-xs outline-none placeholder:text-foreground/60 focus:border-accent focus:ring-2 focus:ring-accent/35"
        />
        {query && (
          <button
            type="button"
            aria-label={tr("mixer.clearSearch")}
            title={tr("mixer.clearSearch")}
            onClick={() => setQuery("")}
            className="absolute right-1.5 top-1/2 inline-flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded text-foreground/60 hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            <X aria-hidden="true" className="h-3.5 w-3.5" />
          </button>
        )}
      </div>

      {(() => {
        const latestTag = updateState?.info?.latest_version;
        const isAvailable =
          updateState?.phase === "available" ||
          updateState?.phase === "downloading" ||
          updateState?.phase === "ready";
        if (!isAvailable || !latestTag || dismissedVersion === latestTag) {
          return null;
        }
        return (
          <div
            data-testid="update-banner"
            role="status"
            className="flex items-center gap-2 rounded-md border border-accent/40 bg-accent/10 px-2 py-1 text-[11px]"
          >
            <span className="min-w-0 flex-1 truncate font-medium">
              {updateState?.info?.message || tr("settings.updates.available", latestTag)}
            </span>
            <button
              type="button"
              onClick={() => openSurface("window-settings")}
              className="shrink-0 text-accent font-medium hover:underline cursor-pointer"
            >
              {tr("settings.updates.viewUpdate")}
            </button>
            {updateState?.info?.release_url && (
              <a
                href={updateState.info.release_url}
                onClick={(event) => {
                  event.preventDefault();
                  void invoke<void>("open_release", {
                    url: updateState.info?.release_url,
                  }).catch(() => {});
                }}
                className="shrink-0 text-accent underline hover:text-accent/80"
              >
                {tr("settings.updates.viewRelease")}
              </a>
            )}
            <button
              type="button"
              aria-label={tr("update.dismiss")}
              title={tr("update.dismiss")}
              onClick={() => setDismissedVersion(latestTag)}
              className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded text-foreground/60 hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent cursor-pointer"
            >
              <X aria-hidden="true" className="h-3 w-3" />
            </button>
          </div>
        );
      })()}

      <SystemOutputRow
        value={volumePct}
        muted={muted}
        thresholds={thresholds}
        onError={(actionError) =>
          setNotice(`${tr("mixer.backendUnavailable")}: ${surfaceErrorMessage(actionError)}`)
        }
      />
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
              <p className="font-semibold">{tr("mixer.connectionAttention")}</p>
              <p className="mt-1 break-words text-foreground/75">{surfaceErrorMessage(error)}</p>
            </div>
            <button
              type="button"
              onClick={retry}
              className="shrink-0 rounded-md border border-foreground/25 bg-background/80 px-2 py-1 font-medium text-foreground transition-colors hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            >
              {tr("mixer.retry")}
            </button>
          </div>
        ) : !sessionsSupported ? (
          <p className="py-8 text-center text-xs text-foreground/70">
            {tr("mixer.unsupported")}
          </p>
        ) : filtered.length === 0 ? (
          <p className="py-8 text-center text-xs text-foreground/70">
            {sessions.length === 0 ? tr("mixer.noSessions") : tr("mixer.noMatches")}
          </p>
        ) : (
          <ul className="space-y-2">
            {filtered.map((session, index) => (
              <motion.li
                key={`${session.id}-${session.name}-${index}`}
                layout
                transition={{ duration: 0.15 }}
              >
                <SessionRow
                  session={session}
                  labelStyle={appLabelStyle}
                  onMute={handleMute}
                  onVolumeError={handleVolumeError}
                />
              </motion.li>
            ))}
          </ul>
        )}
      </div>

      <footer data-testid="surface-footer" className="flex flex-shrink-0 items-center justify-between border-t border-foreground/20 pt-1 text-[11px] text-foreground/70">
        <span>
          {sessionsSupported
            ? `${filtered.length} ${tr("mixer.appSessions")}`
            : tr("mixer.systemOutput")}
        </span>
        {error ? (
          <span className="font-medium text-amber-700 dark:text-amber-300">{tr("mixer.retryConnection")}</span>
        ) : (
          <span>{tr("mixer.escToClose")}</span>
        )}
      </footer>
    </main>
    </LanguageProvider>
  );
}
