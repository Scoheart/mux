/** Flatten a Tauri command error (string | string[] | unknown) for display. */
export function formatError(e: unknown): string {
  return Array.isArray(e) ? e.join("; ") : String(e);
}
