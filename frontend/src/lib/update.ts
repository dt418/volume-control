/** Shape emitted by the Rust update checker and `check_for_updates`. */
export interface UpdateInfoPayload {
  current_version: string;
  latest_version?: string;
  is_update_available: boolean;
  release_url?: string;
  message: string;
}
