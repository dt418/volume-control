import { useEffect, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { useT, type TranslationKey } from "../lib/i18n";
import { invoke } from "../lib/ipc";

export type ConfigLoadNotice =
  | "migrated_from_json"
  | "recovered_from_json"
  | "defaults_after_error"
  | "invalid_external_edit";

const NOTICE_KEYS: Record<ConfigLoadNotice, TranslationKey> = {
  migrated_from_json: "storage.migratedFromJson",
  recovered_from_json: "storage.recoveredFromJson",
  defaults_after_error: "storage.defaultsAfterError",
  invalid_external_edit: "storage.invalidExternalEdit",
};

/** Storage section: read-only canonical INI path + advanced raw-file action. */
export function StorageSection({ notice }: { notice?: ConfigLoadNotice | null }) {
  const [path, setPath] = useState<string | null>(null);
  const tr = useT();

  useEffect(() => {
    let disposed = false;
    void invoke<string>("config_path")
      .then((value) => {
        if (!disposed) setPath(value);
      })
      .catch(() => {
        if (!disposed) setPath(null);
      });
    return () => {
      disposed = true;
    };
  }, []);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{tr("settings.storage")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm">
        {notice && (
          <p
            role="alert"
            className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
          >
            {tr(NOTICE_KEYS[notice])}
          </p>
        )}
        <p className="break-all text-foreground/80">
          {path ?? tr("storage.loadingPath")}
        </p>
        <button
          type="button"
          className="self-start rounded-md border border-foreground/20 px-3 py-1.5"
          onClick={() => void invoke("open_config_location")}
        >
          {tr("settings.openConfig")}
        </button>
        <p className="text-xs text-foreground/60">
          {tr("storage.editingWarning")}
        </p>
      </CardContent>
    </Card>
  );
}
