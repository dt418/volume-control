import { useCallback, useEffect, useState } from "react";
import { invoke, listen } from "../lib/ipc";

export interface AudioSession {
  id: string;
  name: string;
  pct: number;
  muted: boolean;
  active: boolean;
}

export interface BootstrapPayload {
  config: unknown;
  volume_pct: number;
  muted: boolean;
  hotkey_status: unknown[];
  appearance: Record<string, string>;
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
  const [sessionsSupported, setSessionsSupported] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    invoke<BootstrapPayload>("get_bootstrap")
      .then((b) => {
        if (disposed) return;
        setSessions(b.sessions ?? []);
        setVolumePct(b.volume_pct);
        setMuted(b.muted);
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
    sessionsSupported,
    notice,
    setNotice,
    removeSession,
    updateSession,
  };
}
