import { Volume2, VolumeX } from "lucide-react";
import { memo } from "react";

import { AppSlider } from "./AppSlider";
import { appLabel, type AppLabelStyle } from "../lib/appLabel";
import { useT } from "../lib/i18n";
import type { AudioSession } from "./sessionStore";

interface SessionRowProps {
  session: AudioSession;
  labelStyle?: AppLabelStyle;
  onMute: (id: string) => void;
  onVolumeError: (id: string, error: string) => void;
}

export const SessionRow = memo(function SessionRow({
  session,
  labelStyle = "friendly",
  onMute,
  onVolumeError,
}: SessionRowProps) {
  const MuteIcon = session.muted ? VolumeX : Volume2;
  const tr = useT();
  // WASAPI exposes the system-sounds session with an opaque
  // "@%SystemRoot%..." display name. The backend marks it explicitly, so the
  // friendly label survives the session moving between process ids.
  const isSystemSounds = session.system === true;
  const process = session.process ?? session.name;
  const displayName = isSystemSounds
    ? tr("mixer.systemSounds")
    : appLabel(labelStyle, session.display_name, process);
  const description = isSystemSounds
    ? tr("mixer.systemSoundsDescription")
    : displayName;
  const muteLabel = session.muted
    ? `${tr("common.unmute")} ${description}`
    : `${tr("common.mute")} ${description}`;

  return (
    <div
      data-testid="session-row"
      className="flex items-center gap-3 rounded-xl border border-border/40 bg-card/60 px-3 py-2 shadow-sm transition-colors hover:border-accent/40"
    >
      <span className="w-28 shrink-0" title={description}>
        <span className="block truncate text-xs font-medium">{displayName}</span>
        {isSystemSounds && (
          <span className="block truncate text-[9px] leading-tight text-foreground/55">
            {tr("mixer.systemSoundsHint")}
          </span>
        )}
      </span>
      <div className="min-w-0 flex-1">
        <AppSlider
          sessionId={session.id}
          label={description}
          value={session.pct}
          onError={(error) => onVolumeError(session.id, error)}
        />
      </div>
      <button
        type="button"
        aria-label={muteLabel}
        aria-pressed={session.muted}
        title={muteLabel}
        onClick={() => onMute(session.id)}
        className={`shrink-0 rounded-md p-1.5 transition-colors hover:bg-foreground/15 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${
          session.muted ? "text-amber-600 dark:text-amber-400" : "text-foreground/85"
        }`}
      >
        <MuteIcon aria-hidden="true" className="h-4 w-4" />
      </button>
    </div>
  );
});
