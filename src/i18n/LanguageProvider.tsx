import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
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

  const setLanguage = useCallback((lang: Language) => {
    setLanguageState(lang);
    localStorage.setItem("countdown-language", lang);
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
