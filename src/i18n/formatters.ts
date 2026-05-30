import { useMemo, useCallback } from "react";
import { useLanguage } from "./LanguageProvider";

function localeFromLanguage(language: string): string {
  return language === "zh" ? "zh-CN" : "en-US";
}

export function useFormatNumber() {
  const { language } = useLanguage();
  const formatter = useMemo(
    () => new Intl.NumberFormat(localeFromLanguage(language)),
    [language],
  );
  return useCallback(
    (value: number | null | undefined) => {
      if (value == null) return "0";
      return formatter.format(value);
    },
    [formatter],
  );
}

export function useFormatMs() {
  const { t } = useLanguage();
  return useCallback(
    (value: number | null | undefined) => {
      if (value == null) return t("n/a");
      if (value >= 1000) return `${(value / 1000).toFixed(2)} s`;
      return `${value} ms`;
    },
    [t],
  );
}

export function useFormatPercent() {
  const { language } = useLanguage();
  const formatter = useMemo(
    () =>
      new Intl.NumberFormat(localeFromLanguage(language), {
        style: "percent",
        maximumFractionDigits: 1,
      }),
    [language],
  );
  return useCallback(
    (value: number | null | undefined) => formatter.format(value ?? 0),
    [formatter],
  );
}

export function useFormatDateTime() {
  const { language } = useLanguage();
  const formatter = useMemo(
    () =>
      new Intl.DateTimeFormat(localeFromLanguage(language), {
        timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      }),
    [language],
  );
  return useCallback(
    (value: string | null | undefined) => {
      if (!value) return "N/A";
      const date = new Date(value);
      if (Number.isNaN(date.getTime())) return value;
      return formatter.format(date);
    },
    [formatter],
  );
}
