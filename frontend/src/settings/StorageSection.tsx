import { useEffect, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { invoke } from "../lib/ipc";

export type ConfigLoadNotice =
  | "migrated_from_json"
  | "recovered_from_json"
  | "defaults_after_error"
  | "invalid_external_edit";

function noticeText(notice: ConfigLoadNotice): string {
  switch (notice) {
    case "migrated_from_json":
      return "Configuration migrated from legacy JSON to config.ini; the JSON backup was kept.";
    case "recovered_from_json":
      return "Configuration recovered from the legacy JSON backup after an invalid INI edit.";
    case "defaults_after_error":
      return "Configuration files could not be loaded, so validated defaults are active. Your backup was not deleted.";
    case "invalid_external_edit":
      return "An external INI edit was invalid; the last valid configuration remains active. Correct the file and retry.";
  }
}

/** Storage section: read-only canonical INI path + advanced raw-file action. */
export function StorageSection({ notice }: { notice?: ConfigLoadNotice | null }) {
  const [path, setPath] = useState<string | null>(null);

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
        <CardTitle>Storage</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm">
        {notice && (
          <p
            role="alert"
            className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
          >
            {noticeText(notice)}
          </p>
        )}
        <p className="break-all text-foreground/80">
          {path ?? "Loading config path…"}
        </p>
        <button
          type="button"
          className="self-start rounded-md border border-foreground/20 px-3 py-1.5"
          onClick={() => void invoke("open_config_location")}
        >
          Open config.ini (advanced)
        </button>
        <p className="text-xs text-foreground/60">
          Editing config.ini directly is an advanced option; changes reload automatically.
          Legacy config.json is kept as a migration backup.
        </p>
      </CardContent>
    </Card>
  );
}
