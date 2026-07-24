import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useState } from "react";
import i18n, {
  systemLocale,
  type LocalePreference,
  type SupportedLocale,
} from ".";
import { getUiLocale, setUiLocale } from "../lib/api";
import { LegacyLocalizationBridge } from "./LegacyLocalizationBridge";

interface LocaleState {
  preference: LocalePreference;
  locale: SupportedLocale;
  saving: boolean;
  setPreference(preference: LocalePreference): Promise<void>;
}

const LocaleContext = createContext<LocaleState>({
  preference: null,
  locale: systemLocale(),
  saving: false,
  setPreference: async () => undefined,
});

function resolvedLocale(preference: LocalePreference): SupportedLocale {
  return preference ?? systemLocale();
}

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [preference, setPreferenceState] = useState<LocalePreference>(null);
  const [saving, setSaving] = useState(false);
  const locale = resolvedLocale(preference);

  useEffect(() => {
    let active = true;
    getUiLocale()
      .then((saved) => {
        if (active) setPreferenceState(saved);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale;
    void i18n.changeLanguage(locale);
  }, [locale]);

  useEffect(() => {
    const handleSystemLanguage = () => {
      if (preference === null) void i18n.changeLanguage(systemLocale());
    };
    window.addEventListener("languagechange", handleSystemLanguage);
    return () => window.removeEventListener("languagechange", handleSystemLanguage);
  }, [preference]);

  const setPreference = useCallback(async (next: LocalePreference) => {
    const previous = preference;
    setPreferenceState(next);
    setSaving(true);
    try {
      const saved = await setUiLocale(next);
      setPreferenceState(saved);
    } catch (error) {
      setPreferenceState(previous);
      throw error;
    } finally {
      setSaving(false);
    }
  }, [preference]);

  const value = useMemo(
    () => ({ preference, locale, saving, setPreference }),
    [locale, preference, saving, setPreference],
  );

  return (
    <LocaleContext.Provider value={value}>
      {children}
      <LegacyLocalizationBridge locale={locale} />
    </LocaleContext.Provider>
  );
}

export function useLocale(): LocaleState {
  return useContext(LocaleContext);
}
