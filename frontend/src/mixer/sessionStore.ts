import { useCallback, useEffect, useState } from "react";
import { applyAppearance, type AppearancePayload } from "../lib/appearance";
import { invoke, listen } from "../lib/ipc";
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
}

export interface VolumeEvent {
  pct: number;
  muted: boolean;
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

  useEffect(() => {
    let disposed = false;
    invoke<BootstrapPayload>("get_bootstrap")
      .then((b) => {
        if (disposed) return;
        applyAppearance(b.appearance);
        setSessions(b.sessions ?? []);
        setVolumePct(b.volume_pct);
        setMuted(b.muted);
        setThresholds(b.config?.color_thresholds ?? DEFAULT_THRESHOLDS);
        setSessionsSupported(b.sessions_supported);
      })
      .catch(() => {
        // Fail-soft: the surface renders its empty states.
      });

    const unlisteners: Array<() => void> = [];
    void listen<AudioSession[]>("state://sessions", (payload) => {
      setSessions(payload ?? []);
    }).then((unlisten) => unlisteners.push(unlisten));
    void listen<VolumeEvent>("state://volume", (payload) => {
      setVolumePct(payload.pct);
      setMuted(payload.muted);
    }).then((unlisten) => unlisteners.push(unlisten));

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  const removeSession = useCallback((id: string) => {
    setSessions((prev) => prev.filter((s) => s.id !== id));
  }, []);

  const updateSession = useCallback((id: string, patch: Partial<AudioSession>) => {
    setSessions((prev) => prev.map((s) => (s.id === id ? { ...s, ...patch } : s)));
  }, []);

  return {
    sessions: sortSessions(sessions),
    volumePct,
    muted,
    thresholds,
    sessionsSupported,
    notice,
    setNotice,
    removeSession,
    updateSession,
  };
}
