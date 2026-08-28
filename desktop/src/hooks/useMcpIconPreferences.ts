import { useCallback, useEffect, useState } from "react";
import {
  importMcpIconDialog,
  listMcpIconPreferences,
  resetMcpIcon,
  setMcpBuiltinIcon,
} from "../lib/api";
import type { McpIconPreferences } from "../lib/types";
import { formatError } from "../lib/format";

export function useMcpIconPreferences() {
  const [preferences, setPreferences] = useState<McpIconPreferences>({});
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await listMcpIconPreferences();
      setPreferences(next);
      setError(null);
      return next;
    } catch (cause) {
      setError(formatError(cause));
      return {};
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const mutate = useCallback(async (
    assetKey: string,
    operation: () => Promise<McpIconPreferences>,
  ) => {
    setBusyKey(assetKey);
    setError(null);
    try {
      const next = await operation();
      setPreferences(next);
      return next;
    } catch (cause) {
      const message = formatError(cause);
      setError(message);
      throw new Error(message);
    } finally {
      setBusyKey(null);
    }
  }, []);

  const upload = useCallback(async (assetKey: string) => {
    setBusyKey(assetKey);
    setError(null);
    try {
      const next = await importMcpIconDialog(assetKey);
      if (!next) return false;
      setPreferences(next);
      return true;
    } catch (cause) {
      const message = formatError(cause);
      setError(message);
      throw new Error(message);
    } finally {
      setBusyKey(null);
    }
  }, []);

  return {
    preferences,
    busyKey,
    error,
    refresh,
    selectBuiltin: (assetKey: string, iconId: string) =>
      mutate(assetKey, () => setMcpBuiltinIcon(assetKey, iconId)),
    upload,
    reset: (assetKey: string) => mutate(assetKey, () => resetMcpIcon(assetKey)),
  };
}
