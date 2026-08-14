import { invoke } from "./ipc";

export function markSurfaceReady(): Promise<void> {
  return invoke<void>("surface_ready").catch(() => undefined);
}

export function surfaceErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  const message = String(error);
  return message === "undefined" || message === "null" ? "Unknown backend error" : message;
}
