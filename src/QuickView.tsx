import { useEffect, useMemo, useCallback, useState } from "react";
import { getCombinedTodayUsage, type CombinedTodayUsage } from "./desktop";
import { useLanguage } from "./i18n";
import "./QuickView.css";

function useFormatNumber() {
  const { language } = useLanguage();
  const formatter = useMemo(() => new Intl.NumberFormat(language === "zh" ? "zh-CN" : "en-US"), [language]);
  return useCallback((value: number | null | undefined) => {
    if (value == null) {
      return "0";
    }

    return formatter.format(value);
  }, [formatter]);
}

function useFormatClock() {
  const { language } = useLanguage();
  const formatter = useMemo(
    () =>
      new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en-US", {
        hour: "2-digit",
        minute: "2-digit",
      }),
    [language],
  );
  return useCallback(
    (value: string | null | undefined) => {
      if (!value) {
        return "--:--";
      }

      const date = new Date(value);
      if (Number.isNaN(date.getTime())) {
        return "--:--";
      }

      return formatter.format(date);
    },
    [formatter],
  );
}

function QuickView() {
  const { t } = useLanguage();
  const formatNumber = useFormatNumber();
  const formatClock = useFormatClock();
  const [usage, setUsage] = useState<CombinedTodayUsage | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setError(null);
      const data = await getCombinedTodayUsage();
      setUsage(data);
    } catch (refreshError) {
      setError(
        refreshError instanceof Error ? refreshError.message : t("error.refreshUsage"),
      );
    }
  };

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  }, []);

  const providerCards = usage
    ? [
        {
          label: "Claude Code",
          totalTokens: usage.claudeTotalTokens,
          inputTokens: usage.claudeInputTokens,
          outputTokens: usage.claudeOutputTokens,
          requestCount: usage.claudeRequestCount,
          tone: "claude",
        },
        {
          label: "Codex",
          totalTokens: usage.codexTotalTokens,
          inputTokens: usage.codexInputTokens,
          outputTokens: usage.codexOutputTokens,
          requestCount: usage.codexRequestCount,
          tone: "codex",
        },
      ]
    : [];

  return (
    <div className="quick-view-shell">
      <div className="quick-view-panel">
        <div className="quick-view-header">
          <div className="header-copy">
            <div className="header-meta">
              <span className="header-kicker">{t("quickview.today")}</span>
              <span className="header-refresh">
                {t("quickview.updated", formatClock(usage?.lastRefreshAt))}
              </span>
            </div>

            <div className="hero-row">
              <h1>{usage ? formatNumber(usage.combinedTotalTokens) : "--"}</h1>
              <div className="request-meta">
                <strong>{usage?.combinedRequestCount ?? "--"}</strong>
                <span>{t("quickview.totalRequests")}</span>
              </div>
            </div>

            <p>
              {usage
                ? t("quickview.subtitle")
                : t("quickview.loading")}
            </p>
          </div>
        </div>

        {error ? <div className="error-notice">{error}</div> : null}

        {usage ? (
          <div className="quick-view-content">
            <div className="provider-grid">
              {providerCards.map((card) => (
                <section key={card.label} className={`provider-card ${card.tone}`}>
                  <div className="provider-topline">
                    <span className="provider-name">{card.label}</span>
                    <span className="provider-requests">{card.requestCount} req</span>
                  </div>

                  <div className="provider-total">{formatNumber(card.totalTokens)}</div>

                  <div className="provider-metrics">
                    <div className="metric">
                      <span>{t("quickview.metricInput")}</span>
                      <strong>{formatNumber(card.inputTokens)}</strong>
                    </div>
                    <div className="metric">
                      <span>{t("quickview.metricOutput")}</span>
                      <strong>{formatNumber(card.outputTokens)}</strong>
                    </div>
                  </div>
                </section>
              ))}
            </div>
          </div>
        ) : (
          <div className="loading-state">{t("quickview.loadingStats")}</div>
        )}
      </div>
    </div>
  );
}

export default QuickView;
