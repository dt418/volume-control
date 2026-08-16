import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { applyAppearance, type AppearancePayload } from "../lib/appearance";
import { invoke, listen } from "../lib/ipc";
import { markSurfaceReady, surfaceErrorMessage } from "../lib/surface";
import { DEFAULT_THRESHOLDS, type ColorThresholds } from "./SignalRail";

export interface AudioSession {
  id: string;
  name: string;
  pct: number;
  muted: boolean;
  active: boolean;
}

export interface BootstrapPayload {
  /** Serialized `Config`; only `color_thresholds` is consumed by the mixer. */
  config?: { color_thresholds?: ColorThresholds };
  volume_pct: number;
  muted: boolean;
  hotkey_status: unknown[];
  appearance: AppearancePayload;
  sessions: AudioSession[];
  sessions_supported: boolean;
  /** Backend health at mount ("ready" | "degraded"); lets the surface show
   *  a live degraded notice even when the transition event predates the
   *  webview. */
  backend_status?: BackendStatusEvent;
}

export interface VolumeEvent {
  pct: number;
  muted: boolean;
}

/** Host-published backend health (`state://backend`). */
export interface BackendStatusEvent {
  status: "ready" | "degraded";
  error?: string;
}

/** Active sessions first, then by volume descending. */
function sortSessions(sessions: AudioSession[]): AudioSession[] {
  return [...sessions].sort((a, b) => {
    if (a.active !== b.active) return a.active ? -1 : 1;
    return b.pct - a.pct;
  });
}

/**
 * Owns the Mixer's copy of the audio state (SSOT lives in Rust).
 * Loads the bootstrap payload on mount, subscribes to `state://sessions`
 * and `state://volume`, and exposes local mutations for the optimistic
 * slider/mute patterns (the backend does not re-emit on success).
 */
export function useSessions() {
  const [sessions, setSessions] = useState<AudioSession[]>([]);
  const [volumePct, setVolumePct] = useState(0);
  const [muted, setMuted] = useState(false);
  const [thresholds, setThresholds] = useState<ColorThresholds>(DEFAULT_THRESHOLDS);
  const [sessionsSupported, setSessionsSupported] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [bootstrapAttempt, setBootstrapAttempt] = useState(0);
  const volumeEventRevision = useRef(0);

  useEffect(() => {
    let disposed = false;
    const bootstrapVolumeRevision = volumeEventRevision.current;
    const unlisteners: Array<() => void> = [];
    setError(null);

    const listenersReady = Promise.all([
      listen<AudioSession[]>("state://sessions", (payload) => {
        if (!disposed) setSessions(payload ?? []);
      }),
      listen<VolumeEvent>("state://volume", (payload) => {
        if (!disposed) {
          volumeEventRevision.current += 1;
          setVolumePct(payload.pct);
          setMuted(payload.muted);
        }
      }),
      listen<BackendStatusEvent>("state://backend", (payload) => {
        if (!disposed) {
          if (payload.status === "degraded") {
            setNotice(
              `Audio backend temporarily unavailable${payload.error ? `: ${payload.error}` : ""}`,
            );
          } else {
            setNotice(null);
          }
        }
      }),
    ]);

    void listenersReady
      .then((registered) => {
        if (disposed) {
          registered.forEach((unlisten) => unlisten());
          return undefined;
        }
        unlisteners.push(...registered);
        return invoke<BootstrapPayload>("get_bootstrap");
      })
      .then((b) => {
        if (!b || disposed) return;
        setError(null);
        applyAppearance(b.appearance);
        setSessions(b.sessions ?? []);
        if (b.backend_status?.status === "degraded") {
          setNotice(
            `Audio backend temporarily unavailable${b.backend_status.error ? `: ${b.backend_status.error}` : ""}`,
          );
        }
        // A command can complete while bootstrap is in flight. In that case
        // the backend event is newer than the bootstrap snapshot and must win.
        if (volumeEventRevision.current === bootstrapVolumeRevision) {
          setVolumePct(b.volume_pct);
          setMuted(b.muted);
        }
        setThresholds(b.config?.color_thresholds ?? DEFAULT_THRESHOLDS);
        setSessionsSupported(b.sessions_supported);
      })
      .catch((error) => {
        if (!disposed) setError(surfaceErrorMessage(error));
      })
      .finally(() => {
        void markSurfaceReady();
      });

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [bootstrapAttempt]);

  const removeSession = useCallback((id: string) => {
    setSessions((prev) => prev.filter((s) => s.id !== id));
  }, []);

  const updateSession = useCallback((id: string, patch: Partial<AudioSession>) => {
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, ...patch } : s)));
  }, []);
  const retry = useCallback(() => setBootstrapAttempt((attempt) => attempt + 1), []);
  // Sorting is O(n log n) on every render; only re-sort when the list
  // actually changed (perf: state://volume events re-render the surface
  // frequently).
  const sortedSessions = useMemo(() => sortSessions(sessions), [sessions]);

  return {
    sessions: sortedSessions,
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
  };
}
