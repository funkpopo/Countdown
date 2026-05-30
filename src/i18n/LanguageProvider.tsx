import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getUiLanguage, setUiLanguage } from "../desktop";
import { translations, type Language, type Translations } from "./translations";

type LanguageContextValue = {
  language: Language;
  setLanguage: (lang: Language) => void;
  t: (key: string, ...args: string[]) => string;
  _translations: Translations;
};

const LanguageContext = createContext<LanguageContextValue | null>(null);

function getInitialLanguage(): Language {
  if (typeof window === "undefined") return "zh";
  const stored = localStorage.getItem("countdown-language");
  if (stored === "en" || stored === "zh") return stored;
  return "zh";
}

export function LanguageProvider({ children }: { children: ReactNode }) {
  const [language, setLanguageState] = useState<Language>(getInitialLanguage);

  useEffect(() => {
    document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
  }, [language]);

  const setLanguage = useCallback((lang: Language) => {
    setLanguageState(lang);
    localStorage.setItem("countdown-language", lang);
    void setUiLanguage(lang).catch(() => {
      // The app can still render with localStorage language if native sync fails.
    });
  }, []);

  useEffect(() => {
    const stored = localStorage.getItem("countdown-language");
    if (stored === "en" || stored === "zh") {
      void setUiLanguage(stored).catch(() => {
        // Keep the frontend preference even when the native side is unavailable.
      });
    } else {
      void getUiLanguage()
        .then((lang) => {
          if (lang !== "en" && lang !== "zh") return;
          setLanguageState(lang);
          localStorage.setItem("countdown-language", lang);
        })
        .catch(() => {
          // Keep the default language when native preference lookup fails.
        });
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void listen<{ language?: Language }>("ui-language-changed", (event) => {
      const nextLanguage = event.payload?.language;
      if (nextLanguage !== "en" && nextLanguage !== "zh") return;
      setLanguageState(nextLanguage);
      localStorage.setItem("countdown-language", nextLanguage);
    }).then((dispose) => {
      if (disposed) {
        dispose();
        return;
      }
      unlisten = dispose;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const t = useCallback(
    (key: string, ...args: string[]) => {
      const value = translations[language][key];
      if (value == null) {
        return key;
      }
      if (typeof value === "function") {
        return (value as (...a: string[]) => string)(...args);
      }
      return value;
    },
    [language],
  );

  return (
    <LanguageContext.Provider value={{ language, setLanguage, t, _translations: translations[language] }}>
      {children}
    </LanguageContext.Provider>
  );
}

export function useLanguage() {
  const ctx = useContext(LanguageContext);
  if (!ctx) throw new Error("useLanguage must be used within LanguageProvider");
  return ctx;
}

export type { Language } from "./translations";
