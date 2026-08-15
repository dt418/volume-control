import { useEffect, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { invoke } from "../lib/ipc";

/** Storage section (legacy parity): read-only config path + "Open config
 *  file" (the host opens the file and shows the "Editing config — changes
 *  reload automatically" overlay). */
export function StorageSection() {
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
        <p className="break-all text-foreground/80">
          {path ?? "Loading config path…"}
        </p>
        <button
          type="button"
          className="self-start rounded-md border border-foreground/20 px-3 py-1.5"
          onClick={() => void invoke("open_config_location")}
        >
          Open config file
        </button>
        <p className="text-xs text-foreground/60">
          Editing config — changes reload automatically.
        </p>
      </CardContent>
    </Card>
  );
}
