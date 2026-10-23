import { useState, useEffect, useRef } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { invoke, listen } from "../lib/ipc";
import { useT } from "../lib/i18n";
import type { UpdateStatePayload } from "../lib/update";
import pkg from "../../package.json";

export function UpdateCard() {
  const tr = useT();
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);
  const [updateState, setUpdateState] = useState<UpdateStatePayload | null>(null);
  const lastRevision = useRef<number>(-1);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    // Listen first to prevent race with initial state fetch
    void listen<UpdateStatePayload>("state://update", (payload) => {
      if (!disposed && payload.revision > lastRevision.current) {
        lastRevision.current = payload.revision;
        setUpdateState(payload);
        if (payload.phase === "installing") {
          setInstalling(true);
        } else {
          setInstalling(false);
        }
      }
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    // Read initial snapshot without network activity
    void invoke<UpdateStatePayload>("get_update_state")
      .then((state) => {
        if (!disposed && state.revision > lastRevision.current) {
          lastRevision.current = state.revision;
          setUpdateState(state);
        }
      })
      .catch(() => {});

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const handleCheck = async () => {
    setChecking(true);
    setInstallError(null);
    try {
      const state = await invoke<UpdateStatePayload>("check_for_updates", {
        force: true,
      });
      if (state.revision > lastRevision.current) {
        lastRevision.current = state.revision;
        setUpdateState(state);
      }
    } catch {
      // Fail-soft: keep UI responsive
    } finally {
      setChecking(false);
    }
  };

  const handleConfirmInstall = async () => {
    const version = updateState?.info?.latest_version;
    if (!version) return;

    setInstalling(true);
    setInstallError(null);
    try {
      await invoke<void>("install_update", {
        expected_version: version,
      });
    } catch (e: unknown) {
      setInstalling(false);
      setInstallError(e instanceof Error ? e.message : String(e));
    }
  };

  const latestTag = updateState?.info?.latest_version ?? "";
  const phase = updateState?.phase ?? "idle";
  const isCheckingPhase = checking || phase === "checking";

  return (
    <Card data-testid="update-card">
      <CardHeader>
        <CardTitle>{tr("settings.updates.title")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-0.5">
            <span className="text-xs text-foreground/60">{tr("settings.updates.current")}</span>
            <span className="text-sm font-semibold font-mono">
              {updateState?.info?.current_version ? `v${updateState.info.current_version}` : `v${pkg.version}`}
            </span>
          </div>
          <button
            type="button"
            onClick={handleCheck}
            disabled={isCheckingPhase || installing}
            aria-busy={isCheckingPhase}
            className="rounded-md border border-foreground/20 bg-background px-3 py-1.5 text-sm font-medium hover:bg-foreground/5 disabled:opacity-50"
          >
            {isCheckingPhase ? tr("settings.updates.checking") : tr("settings.updates.checkBtn")}
          </button>
        </div>

        {/* Phase: Available or Downloading */}
        {(phase === "available" || phase === "downloading") && (
          <div
            role="status"
            className="rounded-md border border-accent/40 bg-accent/10 p-3 text-sm flex flex-col gap-2 text-foreground"
          >
            <div className="flex items-center gap-2 font-medium">
              <span>{tr("settings.updates.available", latestTag)}</span>
            </div>
            <div className="flex items-center gap-2 text-xs text-foreground/70">
              <span className="inline-block h-2 w-2 rounded-full bg-accent animate-pulse" />
              <span>{tr("settings.updates.downloading")}</span>
            </div>
            {updateState?.info?.release_url && (
              <div className="mt-1 flex items-center gap-3">
                <a
                  href={updateState.info.release_url}
                  onClick={(event) => {
                    event.preventDefault();
                    void invoke<void>("open_release", {
                      url: updateState.info?.release_url,
                    }).catch(() => {});
                  }}
                  className="text-xs text-accent underline hover:text-accent/80 font-medium"
                >
                  {tr("settings.updates.viewRelease")}
                </a>
              </div>
            )}
          </div>
        )}

        {/* Phase: Ready to install */}
        {phase === "ready" && (
          <div
            role="status"
            className="rounded-md border border-emerald-500/40 bg-emerald-500/10 p-3 text-sm flex flex-col gap-2 text-foreground"
          >
            <div className="flex items-center gap-2 font-medium text-emerald-600 dark:text-emerald-400">
              <span>{tr("settings.updates.ready", latestTag)}</span>
            </div>

            {!updateState?.install_supported ? (
              <p className="text-xs text-foreground/70">{tr("settings.updates.managedExternally")}</p>
            ) : !showConfirm ? (
              <div className="flex items-center gap-3 mt-1">
                <button
                  type="button"
                  onClick={() => setShowConfirm(true)}
                  disabled={installing}
                  className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-emerald-700 disabled:opacity-50"
                >
                  {tr("settings.updates.installRestart")}
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
                    className="text-xs text-foreground/70 underline hover:text-foreground font-medium"
                  >
                    {tr("settings.updates.viewRelease")}
                  </a>
                )}
              </div>
            ) : (
              <div className="mt-2 rounded-md border border-foreground/15 bg-background/50 p-3 flex flex-col gap-2">
                <p className="text-xs text-foreground/80 leading-relaxed">
                  {tr("settings.updates.confirmBody", latestTag)}
                </p>
                <div className="flex items-center gap-2 mt-1">
                  <button
                    type="button"
                    onClick={handleConfirmInstall}
                    disabled={installing}
                    className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-emerald-700 disabled:opacity-50"
                  >
                    {installing ? tr("settings.updates.checking") : tr("settings.updates.confirmInstall")}
                  </button>
                  <button
                    type="button"
                    onClick={() => setShowConfirm(false)}
                    disabled={installing}
                    className="rounded-md border border-foreground/20 bg-background px-3 py-1.5 text-xs font-medium hover:bg-foreground/5 disabled:opacity-50"
                  >
                    {tr("settings.updates.later")}
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Phase: Installing */}
        {phase === "installing" && (
          <div
            role="status"
            className="rounded-md border border-accent/30 bg-accent/5 p-3 text-sm flex items-center gap-2"
          >
            <span className="inline-block h-3 w-3 rounded-full border-2 border-accent border-t-transparent animate-spin" />
            <span className="font-medium text-foreground">{tr("settings.updates.downloading")}</span>
          </div>
        )}

        {/* Phase: Idle with Up to Date */}
        {phase === "idle" && updateState?.info && !updateState.info.is_update_available && (
          <div
            role="status"
            className="rounded-md border border-foreground/10 bg-foreground/5 p-3 text-sm flex items-center gap-2 text-foreground/80"
          >
            <span>{tr("settings.updates.upToDate")}</span>
          </div>
        )}

        {/* Phase: Error */}
        {(phase === "error" || installError || updateState?.error) && (
          <div
            role="alert"
            className="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm flex flex-col gap-2 text-foreground"
          >
            <span className="font-medium text-destructive">
              {tr("settings.updates.installFailed", installError || updateState?.error || "")}
            </span>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleCheck}
                disabled={isCheckingPhase}
                className="text-xs font-medium underline text-destructive hover:opacity-80"
              >
                {tr("settings.updates.retry")}
              </button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
