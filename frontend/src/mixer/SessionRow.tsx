import { Volume2, VolumeX } from "lucide-react";
import { memo } from "react";

import { AppSlider } from "./AppSlider";
import type { AudioSession } from "./sessionStore";

interface SessionRowProps {
  session: AudioSession;
  onMute: (id: string) => void;
  onVolumeError: (id: string, error: string) => void;
}

export const SessionRow = memo(function SessionRow({
  session,
  onMute,
  onVolumeError,
}: SessionRowProps) {
  const MuteIcon = session.muted ? VolumeX : Volume2;
  return (
    <div
      data-testid="session-row"
      className="flex items-center gap-3 rounded-lg border border-foreground/20 bg-background/88 px-3 py-2 shadow-sm"
    >
      <span className="w-28 shrink-0 truncate text-xs font-medium" title={session.name}>
        {session.name}
      </span>
      <div className="flex-1">
        <AppSlider
          sessionId={session.id}
          value={session.pct}
          onError={(error) => onVolumeError(session.id, error)}
        />
      </div>
      <button
        type="button"
        aria-label={session.muted ? `Unmute ${session.name}` : `Mute ${session.name}`}
        onClick={() => onMute(session.id)}
        className="shrink-0 rounded-md p-1.5 text-foreground/85 transition-colors hover:bg-foreground/15 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
      >
        <MuteIcon className="h-4 w-4" />
      </button>
    </div>
  );
});
