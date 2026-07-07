"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { DEFAULT_LANGUAGE, isSupportedLanguage, languageDir } from "./languages";
import en from "./translations/en";
import tr from "./translations/tr";
import fr from "./translations/fr";
import de from "./translations/de";
import ru from "./translations/ru";
import es from "./translations/es";
import ar from "./translations/ar";
import zh from "./translations/zh";
import ja from "./translations/ja";
import ko from "./translations/ko";

type Dictionary = typeof en;

const DICTIONARIES: Record<string, Dictionary> = { en, tr, fr, de, ru, es, ar, zh, ja, ko };

const STORAGE_KEY = "steward.language";

interface LanguageContextValue {
  language: string;
  setLanguage: (code: string) => void;
  t: (key: keyof Dictionary, vars?: Record<string, string>) => string;
}

const LanguageContext = createContext<LanguageContextValue | null>(null);

function readStoredLanguage(): string {
  if (typeof window === "undefined") return DEFAULT_LANGUAGE;
  const stored = window.localStorage.getItem(STORAGE_KEY);
  return stored && isSupportedLanguage(stored) ? stored : DEFAULT_LANGUAGE;
}

export function LanguageProvider({ children }: { readonly children: React.ReactNode }) {
  const [language, setLanguageState] = useState(DEFAULT_LANGUAGE);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setLanguageState(readStoredLanguage());
    }, 0);
    return () => window.clearTimeout(timer);
  }, []);

  useEffect(() => {
    document.documentElement.lang = language;
    document.documentElement.dir = languageDir(language);
  }, [language]);

  const setLanguage = useCallback((code: string) => {
    if (!isSupportedLanguage(code)) return;
    setLanguageState(code);
    window.localStorage.setItem(STORAGE_KEY, code);
  }, []);

  const t = useCallback(
    (key: keyof Dictionary, vars?: Record<string, string>) => {
      const dictionary = DICTIONARIES[language] ?? en;
      let text = dictionary[key] ?? en[key] ?? key;
      if (vars) {
        for (const [name, value] of Object.entries(vars)) {
          text = text.replaceAll(`{${name}}`, value);
        }
      }
      return text;
    },
    [language]
  );

  const value = useMemo(() => ({ language, setLanguage, t }), [language, setLanguage, t]);

  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

export function useTranslation() {
  const context = useContext(LanguageContext);
  if (!context) {
    throw new Error("useTranslation must be used within a LanguageProvider");
  }
  return context;
}
