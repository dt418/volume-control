import { useEffect, useState } from "react";
import { applyAppearance } from "../lib/appearance";
import { listen } from "../lib/ipc";
import { markSurfaceReady } from "../lib/surface";
import {
  DEFAULT_THRESHOLDS,
  SignalRail,
  type ColorThresholds,
} from "../mixer/SignalRail";

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

export function OverlaySurface() {
  const [payload, setPayload] = useState<OverlayPayload | null>(null);

  useEffect(() => {
    // The host creates the HUD hidden (`visible(false)`); the window only
    // becomes visible after `surface_ready` (same contract as the mixer).
    void markSurfaceReady();
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    void listen<OverlayPayload>("state://overlay", (p) => {
      if (disposed) return;
      setPayload(p);
      const resolved =
        p.theme === "Dark"
          ? "dark"
          : p.theme === "Light"
            ? "light"
            : window.matchMedia("(prefers-color-scheme: dark)").matches
              ? "dark"
              : "light";
      applyAppearance({
        theme_resolved: resolved,
        material: p.material,
        motion: p.motion,
        accent: p.accent,
      });
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    });
    return () => {
      disposed = true;
      unlisteners.forEach((un) => un());
    };
  }, []);

  if (!payload) return null;

  const thresholds: ColorThresholds = {
    green_up_to: payload.green_up_to,
    blue_up_to: payload.blue_up_to,
    orange_up_to: payload.orange_up_to,
  };

  return (
    <div
      data-surface="overlay"
      className="pointer-events-none flex h-screen w-screen items-center justify-center bg-transparent"
    >
      <div
        data-testid="overlay-card"
        className="flex w-[336px] flex-col gap-2 rounded-xl border border-border/60 bg-background/88 px-4 py-3 shadow-lg backdrop-blur-md"
      >
        {payload.text ? (
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
                {payload.pct}%
              </span>
            </div>
            <SignalRail
              value={payload.pct}
              muted={payload.muted}
              thresholds={thresholds}
            />
          </>
        )}
      </div>
    </div>
  );
}
