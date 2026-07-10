import { useCallback, useEffect, useRef, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import type { Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// Remember the version the user clicked "稍后" on, so the startup check stops
// nagging about it. A manual check (点版本号) always ignores this.
const DISMISS_KEY = "mux-update-dismissed";

export type UpdatePhase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "downloading"; percent: number | null }
  | { kind: "ready"; version: string }
  | { kind: "error"; message: string };

export interface UpdaterState {
  phase: UpdatePhase;
  /** Check the release channel. Returns what happened so callers can toast. */
  checkNow: (opts?: { manual?: boolean }) => Promise<"available" | "latest" | "error">;
  /** Download + stage the available update (with progress in `phase`). */
  download: () => Promise<void>;
  /** Relaunch into the updated build. */
  restart: () => Promise<void>;
  /** "稍后" — hide the banner and don't re-prompt for this version on startup. */
  dismiss: () => void;
  /** "下次启动时" — update is staged; just hide the banner. */
  later: () => void;
}

export function useUpdater(): UpdaterState {
  const [phase, setPhase] = useState<UpdatePhase>({ kind: "idle" });
  const updateRef = useRef<Update | null>(null);

  const checkNow = useCallback(
    async ({ manual = false }: { manual?: boolean } = {}) => {
      setPhase({ kind: "checking" });
      try {
        const update = await check();
        if (update) {
          if (!manual && localStorage.getItem(DISMISS_KEY) === update.version) {
            // User already said "稍后" to this exact version — stay quiet.
            setPhase({ kind: "idle" });
            return "latest" as const;
          }
          updateRef.current = update;
          setPhase({
            kind: "available",
            version: update.version,
            notes: update.body?.trim() || null,
          });
          return "available" as const;
        }
        setPhase({ kind: "idle" });
        return "latest" as const;
      } catch (e) {
        // A silent startup check failing (offline, rate-limit…) should never
        // surface UI; only manual checks report the error.
        setPhase(manual ? { kind: "error", message: String(e) } : { kind: "idle" });
        return "error" as const;
      }
    },
    [],
  );

  const download = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    setPhase({ kind: "downloading", percent: null });
    let total: number | null = null;
    let received = 0;
    try {
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started") {
          total = ev.data.contentLength ?? null;
        } else if (ev.event === "Progress") {
          received += ev.data.chunkLength;
          if (total) {
            setPhase({
              kind: "downloading",
              percent: Math.min(99, Math.round((received / total) * 100)),
            });
          }
        } else if (ev.event === "Finished") {
          setPhase({ kind: "downloading", percent: 100 });
        }
      });
      setPhase({ kind: "ready", version: update.version });
    } catch (e) {
      setPhase({ kind: "error", message: String(e) });
    }
  }, []);

  const restart = useCallback(async () => {
    await relaunch();
  }, []);

  const dismiss = useCallback(() => {
    const v = updateRef.current?.version;
    if (v) localStorage.setItem(DISMISS_KEY, v);
    setPhase({ kind: "idle" });
  }, []);

  const later = useCallback(() => setPhase({ kind: "idle" }), []);

  // Silent check shortly after launch — delayed so it never competes with the
  // startup scan, and any failure is swallowed by checkNow itself.
  useEffect(() => {
    const t = setTimeout(() => void checkNow(), 2500);
    return () => clearTimeout(t);
  }, [checkNow]);

  return { phase, checkNow, download, restart, dismiss, later };
}
