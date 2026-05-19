import { useEffect, useState } from "react";
import { getCombinedTodayUsage, type CombinedTodayUsage } from "./desktop";
import "./QuickView.css";

function formatNumber(value: number | null | undefined) {
  if (value == null) {
    return "0";
  }

  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }

  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}k`;
  }

  return new Intl.NumberFormat("en-US").format(value);
}

function formatClock(value: string | null | undefined) {
  if (!value) {
    return "--:--";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "--:--";
  }

  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function QuickView() {
  const [usage, setUsage] = useState<CombinedTodayUsage | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setError(null);
      const data = await getCombinedTodayUsage();
      setUsage(data);
    } catch (refreshError) {
      setError(
        refreshError instanceof Error ? refreshError.message : "Failed to load usage data.",
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
              <span className="header-kicker">TODAY</span>
              <span className="header-refresh">
                {formatClock(usage?.lastRefreshAt)} 更新
              </span>
            </div>

            <div className="hero-row">
              <h1>{usage ? formatNumber(usage.combinedTotalTokens) : "--"}</h1>
              <div className="request-meta">
                <strong>{usage?.combinedRequestCount ?? "--"}</strong>
                <span>总请求</span>
              </div>
            </div>

            <p>
              {usage
                ? "Claude Code 与 Codex 今日总消耗"
                : "正在读取本地用量统计"}
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
                      <span>输入</span>
                      <strong>{formatNumber(card.inputTokens)}</strong>
                    </div>
                    <div className="metric">
                      <span>输出</span>
                      <strong>{formatNumber(card.outputTokens)}</strong>
                    </div>
                  </div>
                </section>
              ))}
            </div>
          </div>
        ) : (
          <div className="loading-state">正在载入今日统计…</div>
        )}
      </div>
    </div>
  );
}

export default QuickView;
