import { useCallback, useEffect, useState } from "react";
import { getProxySettings, setProxySettings } from "../lib/api";
import type { ProxySettings } from "../lib/types";

const EMPTY_SETTINGS: ProxySettings = { proxy_url: null };

export function useNetworkSettings() {
  const [settings, setSettings] = useState<ProxySettings>(EMPTY_SETTINGS);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    getProxySettings()
      .then((value) => {
        if (active) setSettings(value);
      })
      .catch(() => {})
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const save = useCallback(async (proxyUrl: string | null) => {
    const value = await setProxySettings(proxyUrl);
    setSettings(value);
    return value;
  }, []);

  return { settings, loading, save };
}
