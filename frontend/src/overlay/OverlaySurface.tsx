import { useEffect, useState } from "react";
import { applyAppearance, type AppearancePayload } from "../lib/appearance";
import { invoke, listen } from "../lib/ipc";
import { markSurfaceReady } from "../lib/surface";
import { DEFAULT_THRESHOLDS, SignalRail, type ColorThresholds } from "../mixer/SignalRail";

interface OverlayPayload {
  text: string | null;
  pct: number;
  muted: boolean;
  green_up_to: number;
  blue_up_to: number;
  orange_up_to: number;
  theme: string;
  material: string;
  motion: string;
  accent: string;
}

interface OverlayBootstrap {
  volume_pct: number;
  muted: boolean;
  config?: { color_thresholds?: ColorThresholds };
  appearance: AppearancePayload;
}

function resolveTheme(theme: string): string {
  if (theme === "Dark") return "dark";
  if (theme === "Light") return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function OverlaySurface() {
  const [payload, setPayload] = useState<OverlayPayload | null>(null);
  const [initial, setInitial] = useState<OverlayBootstrap | null>(null);
  const [live, setLive] = useState<{ pct: number; muted: boolean } | null>(null);

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    void listen<OverlayPayload>("state://overlay", (p) => {
      if (disposed) return;
      setPayload(p);
      applyAppearance({
        theme_resolved: resolveTheme(p.theme),
        material: p.material,
        motion: p.motion,
        accent: p.accent,
      });
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    });
    void listen<{ pct: number; muted: boolean }>("state://volume", (v) => {
      if (!disposed) setLive(v);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    });

    void invoke<OverlayBootstrap>("get_bootstrap")
      .then((b) => {
        if (!b || disposed) return;
        setInitial(b);
        applyAppearance(b.appearance);
      })
      .catch(() => {
        // Backend unavailable: keep the overlay transparent; the host
        // auto-hide still closes it.
      })
      .finally(() => {
        // Show the window only after the initial content is ready so the
        // overlay never appears empty (sync fix).
        void markSurfaceReady();
      });

    return () => {
      disposed = true;
      unlisteners.forEach((un) => un());
    };
  }, []);

  const pct = live?.pct ?? payload?.pct ?? initial?.volume_pct ?? 0;
  const muted = live?.muted ?? payload?.muted ?? initial?.muted ?? false;
  const thresholds: ColorThresholds =
    payload && payload.green_up_to !== undefined
      ? {
          green_up_to: payload.green_up_to,
          blue_up_to: payload.blue_up_to,
          orange_up_to: payload.orange_up_to,
        }
      : initial?.config?.color_thresholds ?? DEFAULT_THRESHOLDS;

  if (!payload && !initial) return null;

  return (
    <div
      data-surface="overlay"
      className="pointer-events-none flex h-screen w-screen items-center justify-center bg-transparent"
    >
      <div
        data-testid="overlay-card"
        className="flex w-full flex-col gap-2 rounded-xl border border-border/60 bg-background/88 px-4 py-3 shadow-lg backdrop-blur-md"
      >
        {payload?.text ? (
          <p className="text-center text-sm font-medium text-foreground">
            {payload.text}
          </p>
        ) : (
          <>
            <div className="flex items-baseline justify-between">
              <span className="text-xs uppercase tracking-wide text-foreground/70">
                Volume
              </span>
              <span className="text-2xl font-semibold tabular-nums text-foreground">
                {pct}%
              </span>
            </div>
            <SignalRail value={pct} muted={muted} thresholds={thresholds} />
          </>
        )}
      </div>
    </div>
  );
}
