import { useEffect, useMemo, useCallback, useState } from "react";
import {
  getQuickViewSummary,
  openMainPage,
  quickViewPointerEnter,
  quickViewPointerLeave,
  type QuickViewSummary,
} from "./desktop";
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

function useFormatPercent() {
  const { language } = useLanguage();
  const formatter = useMemo(
    () =>
      new Intl.NumberFormat(language === "zh" ? "zh-CN" : "en-US", {
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

function QuickView() {
  const { t } = useLanguage();
  const formatNumber = useFormatNumber();
  const formatClock = useFormatClock();
  const formatPercent = useFormatPercent();
  const [summary, setSummary] = useState<QuickViewSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setError(null);
      const data = await getQuickViewSummary();
      setSummary(data);
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

  const usage = summary?.usage ?? null;
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

  const handleOpenMain = () => {
    void openMainPage("overview", "today").catch((openError) => {
      setError(openError instanceof Error ? openError.message : t("error.openMainWindow"));
    });
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }

    event.preventDefault();
    handleOpenMain();
  };

  return (
    <div
      className="quick-view-shell"
      role="button"
      tabIndex={0}
      onClick={handleOpenMain}
      onKeyDown={handleKeyDown}
      onMouseEnter={() => {
        void quickViewPointerEnter();
      }}
      onMouseLeave={() => {
        void quickViewPointerLeave();
      }}
    >
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

        {summary ? (
          <>
            <div className="runtime-strip">
              <div className={`compat-status ${summary.compatApiRunning ? "running" : "stopped"}`}>
                <span className="status-dot" />
                <div>
                  <span>{t("quickview.compatApi")}</span>
                  <strong>
                    {summary.compatApiRunning ? t("quickview.running") : t("quickview.stopped")}
                  </strong>
                </div>
              </div>
              <div className="compat-address">
                <span>{t("quickview.profiles", formatNumber(summary.compatApiProfilesCount))}</span>
                <strong>{summary.compatApiListenAddress}</strong>
              </div>
            </div>

            <div className="quick-metric-grid">
              <section className="quick-stat">
                <span>{t("quickview.lastHour")}</span>
                <strong>{formatNumber(summary.recentOneHourRequestCount)}</strong>
                <small>{t("quickview.recentRequests")}</small>
              </section>
              <section className="quick-stat">
                <span>{t("quickview.errors")}</span>
                <strong>{formatNumber(summary.recentOneHourErrorCount)}</strong>
                <small>{t("quickview.recentErrors")}</small>
              </section>
              <section className="quick-stat">
                <span>{t("quickview.errorRate")}</span>
                <strong>{formatPercent(summary.recentOneHourErrorRate)}</strong>
                <small>{t("quickview.recentErrors")}</small>
              </section>
            </div>
          </>
        ) : null}

        {usage ? (
          <div className="quick-view-content">
            <div className="provider-grid">
              {providerCards.map((card) => (
                <section key={card.label} className={`provider-card ${card.tone}`}>
                  <div className="provider-topline">
                    <span className="provider-name">{card.label}</span>
                    <span className="provider-requests">
                      {t("quickview.providerRequests", formatNumber(card.requestCount))}
                    </span>
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
