import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { invoke, listen } from "../lib/ipc";
import { useT } from "../lib/i18n";
import { notifyUpdate } from "../lib/notify";
import type { UpdateInfoPayload } from "../lib/update";
import pkg from "../../package.json";

export function UpdateCard() {
  const tr = useT();
  const [checking, setChecking] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfoPayload | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<UpdateInfoPayload>("state://update", (payload) => {
      setUpdateInfo(payload);
      void notifyUpdate(payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const handleCheck = async () => {
    setChecking(true);
    try {
      const result = await invoke<UpdateInfoPayload>("check_for_updates", {
        force: true,
      });
      setUpdateInfo(result);
      void notifyUpdate(result);
    } catch {
      // Fail-soft: keep UI responsive
    } finally {
      setChecking(false);
    }
  };

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
              {updateInfo?.current_version ? `v${updateInfo.current_version}` : `v${pkg.version}`}
            </span>
          </div>
          <button
            type="button"
            onClick={handleCheck}
            disabled={checking}
            aria-busy={checking}
            className="rounded-md border border-foreground/20 bg-background px-3 py-1.5 text-sm font-medium hover:bg-foreground/5 disabled:opacity-50"
          >
            {checking ? tr("settings.updates.checking") : tr("settings.updates.checkBtn")}
          </button>
        </div>

        {updateInfo && (
          <div
            role="status"
            className={`rounded-md p-3 text-sm flex flex-col gap-1 ${
              updateInfo.is_update_available
                ? "border border-accent/40 bg-accent/10 text-foreground"
                : "border border-foreground/10 bg-foreground/5 text-foreground/80"
            }`}
          >
            <div className="flex items-center gap-2 font-medium">
              <span>{updateInfo.message}</span>
            </div>
            {updateInfo.is_update_available && updateInfo.release_url && (
              <div className="mt-1 flex items-center gap-3">
                <a
                  href={updateInfo.release_url}
                  onClick={(event) => {
                    event.preventDefault();
                    void invoke<void>("open_release", {
                      url: updateInfo.release_url,
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
      </CardContent>
    </Card>
  );
}
