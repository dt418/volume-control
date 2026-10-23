/** State phases emitted by the Rust update coordinator. */
export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "installing"
  | "error";

/** Release information within UpdateState. */
export interface UpdateInfoPayload {
  current_version: string;
  latest_version: string | null;
  is_update_available: boolean;
  release_url: string | null;
  message: string;
}

/** Complete update coordinator snapshot payload emitted via state://update. */
export interface UpdateStatePayload {
  revision: number;
  phase: UpdatePhase;
  info: UpdateInfoPayload | null;
  error: string | null;
  install_supported: boolean;
}
