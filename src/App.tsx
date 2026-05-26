import { lazy, memo, Suspense, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getClaudeCodeOverview,
  getCodexOverview,
  getCombinedUsageTotal,
  getUsageHistogram,
  getCombinedUsage,
  getDatabaseSummary,
  initializeLocalDatabase,
  listFilteredRequests,
  type ClaudeOverview,
  type CodexOverview,
  type CombinedUsage,
  type DailyUsageRecord,
  type DatabaseSummary,
  type RequestRecordListItem,
  type UsageHistogram,
} from "./desktop";
import { useLanguage } from "./i18n";
import "./App.css";

const Requests = lazy(() => import("./Requests"));
const Settings = lazy(() => import("./Settings"));

function useFormatNumber() {
  const { language } = useLanguage();
  const formatter = useMemo(() => new Intl.NumberFormat(language === "zh" ? "zh-CN" : "en-US"), [language]);
  return useCallback((value: number | null | undefined) => {
    if (value == null) return "0";
    return formatter.format(value);
  }, [formatter]);
}

function useFormatMs() {
  const { t } = useLanguage();
  return useCallback((value: number | null | undefined) => {
    if (value == null) return t("n/a");
    if (value >= 1000) return `${(value / 1000).toFixed(2)} s`;
    return `${value} ms`;
  }, [t]);
}

function renderUsageStat(label: string, value: string) {
  return (
    <div className="stat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function GridIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect width="7" height="7" x="3" y="3" rx="1" />
      <rect width="7" height="7" x="14" y="3" rx="1" />
      <rect width="7" height="7" x="3" y="14" rx="1" />
      <rect width="7" height="7" x="14" y="14" rx="1" />
    </svg>
  );
}

function ListIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="8" y1="6" x2="21" y2="6" />
      <line x1="8" y1="12" x2="21" y2="12" />
      <line x1="8" y1="18" x2="21" y2="18" />
      <line x1="3" y1="6" x2="3.01" y2="6" />
      <line x1="3" y1="12" x2="3.01" y2="12" />
      <line x1="3" y1="18" x2="3.01" y2="18" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

function ChevronLeftIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="m15 18-6-6 6-6" />
    </svg>
  );
}

type Period = "today" | "week" | "month" | "total";
type HistogramMetric = "tokens" | "requests";
const OVERVIEW_RECENT_PAGE_SIZE = 10;

function getDateRangeForPeriod(period: Period): { startDate: string; endDate: string } {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const y = now.getFullYear();
  const m = pad(now.getMonth() + 1);
  const d = pad(now.getDate());
  const endDate = `${y}-${m}-${d}`;

  if (period === "today") {
    return { startDate: endDate, endDate };
  }

  if (period === "week") {
    const start = new Date(now);
    const day = start.getDay();
    const diff = day === 0 ? 6 : day - 1;
    start.setDate(start.getDate() - diff);
    return {
      startDate: `${start.getFullYear()}-${pad(start.getMonth() + 1)}-${pad(start.getDate())}`,
      endDate,
    };
  }

  const start = new Date(now.getFullYear(), now.getMonth(), 1);
  return {
    startDate: `${start.getFullYear()}-${pad(start.getMonth() + 1)}-${pad(start.getDate())}`,
    endDate,
  };
}

type OverviewPageProps = {
  databaseSummary: DatabaseSummary | null;
  codexOverview: CodexOverview | null;
  claudeOverview: ClaudeOverview | null;
  periodUsage: CombinedUsage | null;
  histogram: UsageHistogram | null;
  period: Period;
  histogramMetric: HistogramMetric;
  refreshToken: number;
  error: string | null;
  isPending: boolean;
  onPeriodChange: (period: Period) => void;
  onHistogramMetricChange: (metric: HistogramMetric) => void;
  onRefresh: () => void;
  onInitializeDatabase: () => void;
};

const OverviewPage = memo(function OverviewPage({
  databaseSummary,
  codexOverview,
  claudeOverview,
  periodUsage,
  histogram,
  period,
  histogramMetric,
  refreshToken,
  error,
  isPending,
  onPeriodChange,
  onHistogramMetricChange,
  onRefresh,
  onInitializeDatabase,
}: OverviewPageProps) {
  const { t } = useLanguage();
  const formatNumber = useFormatNumber();
  const formatMs = useFormatMs();
  const pageRef = useRef<HTMLDivElement | null>(null);
  const scrollTopRef = useRef(0);
  const hasRestoredScrollRef = useRef(false);

  const codexTodayUsage: DailyUsageRecord | null = codexOverview?.todayUsage ?? null;
  const claudeTodayUsage: DailyUsageRecord | null = claudeOverview?.todayUsage ?? null;
  const tables = databaseSummary?.tables ?? [];
  const hasTables = tables.length > 0;

  useLayoutEffect(() => {
    if (!hasRestoredScrollRef.current) {
      hasRestoredScrollRef.current = true;
      return;
    }

    const page = pageRef.current;
    if (!page) {
      return;
    }

    page.scrollTop = scrollTopRef.current;
  }, [refreshToken]);

  return (
    <div
      ref={pageRef}
      className="page"
      onScroll={(event) => {
        scrollTopRef.current = event.currentTarget.scrollTop;
      }}
    >
      <div className="page-sticky">
        <div className="page-header">
          <div>
            <h1>{t("overview.title")}</h1>
            <p className="page-meta">
              {t("overview.meta",
                formatNumber(databaseSummary?.providerProfiles.length),
                formatNumber(codexOverview?.requestCount),
                formatNumber(claudeOverview?.requestCount))}
            </p>
          </div>
          <div className="actions">
            <button type="button" onClick={onRefresh} disabled={isPending}>
              {t("overview.refresh")}
            </button>
            <button
              type="button"
              className="ghost"
              onClick={onInitializeDatabase}
              disabled={isPending}
            >
              {t("overview.initDb")}
            </button>
          </div>
        </div>

        {error ? <div className="notice error">{error}</div> : null}

        <div className="period-tabs">
          {(["today", "week", "month", "total"] as const).map((p) => (
            <button
              key={p}
              className={"period-tab" + (period === p ? " active" : "")}
              onClick={() => onPeriodChange(p)}
            >
              {t(`tab.${p}`)}
            </button>
          ))}
          <span className="period-range">
            {period === "total"
              ? t("tab.total.range")
              : periodUsage
                ? `${periodUsage.startDate} ~ ${periodUsage.endDate}`
                : "..."}
          </span>
        </div>
      </div>

      <div className="grid">
        <div className="card">
          <div className="card-header">
            <h2>{t("codex.title")}</h2>
            <span className="card-meta">
              {t("codex.sessions", formatNumber(codexOverview?.sessionCount))}
            </span>
          </div>
          <div className="stats">
            {renderUsageStat(t("stat.input"), formatNumber(periodUsage?.codexInputTokens))}
            {renderUsageStat(t("stat.output"), formatNumber(periodUsage?.codexOutputTokens))}
            {renderUsageStat(t("stat.total"), formatNumber(periodUsage?.codexTotalTokens))}
            {renderUsageStat(t("stat.requests"), formatNumber(periodUsage?.codexRequestCount))}
            {renderUsageStat(t("stat.avgTtft"), formatMs(codexTodayUsage?.avgTtftMs ?? null))}
            {renderUsageStat(t("stat.avgDuration"), formatMs(codexTodayUsage?.avgDurationMs ?? null))}
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <h2>{t("claude.title")}</h2>
            <span className="card-meta">
              {t("codex.sessions", formatNumber(claudeOverview?.sessionCount))}
            </span>
          </div>
          <div className="stats">
            {renderUsageStat(t("stat.input"), formatNumber(periodUsage?.claudeInputTokens))}
            {renderUsageStat(t("stat.output"), formatNumber(periodUsage?.claudeOutputTokens))}
            {renderUsageStat(t("stat.total"), formatNumber(periodUsage?.claudeTotalTokens))}
            {renderUsageStat(t("stat.requests"), formatNumber(periodUsage?.claudeRequestCount))}
            {renderUsageStat(t("stat.avgTtft"), formatMs(claudeTodayUsage?.avgTtftMs ?? null))}
            {renderUsageStat(t("stat.avgDuration"), formatMs(claudeTodayUsage?.avgDurationMs ?? null))}
          </div>
        </div>

        {period !== "total" ? (
          <UsageHistogramPanel
            histogram={histogram}
            metric={histogramMetric}
            onMetricChange={onHistogramMetricChange}
          />
        ) : null}

        <RecentRequestsPanel
          provider="codex"
          title={t("recent.codex")}
          dataDir={codexOverview?.dataDir}
          emptyText={t("recent.empty.codex")}
          ready={codexOverview !== null}
          refreshToken={refreshToken}
        />

        <RecentRequestsPanel
          provider="claude_code"
          title={t("recent.claude")}
          dataDir={claudeOverview?.dataDir}
          emptyText={t("recent.empty.claude")}
          ready={claudeOverview !== null}
          refreshToken={refreshToken}
        />

        <div className="card wide">
          <div className="card-header">
            <h2>{t("storage.title")}</h2>
            <span className="card-meta">{t("storage.tables", formatNumber(tables.length))}</span>
          </div>
          <div className="storage-summary">
            {renderUsageStat(
              t("storage.initializedAt"),
              databaseSummary?.initializedAt ? formatDate(databaseSummary.initializedAt) : t("n/a"),
            )}
            {renderUsageStat(t("storage.profiles"), formatNumber(databaseSummary?.providerProfiles.length))}
          </div>
          <div className="table-grid">
            {hasTables ? tables.map((table) => (
              <div key={table.tableName} className="table-card">
                <strong>{table.tableName}</strong>
                <span>{formatNumber(table.rowCount)} {t("storage.rows")}</span>
              </div>
            )) : <p className="empty">{databaseSummary ? t("storage.empty") : t("storage.waiting")}</p>}
          </div>
        </div>
      </div>
    </div>
  );
});

const UsageHistogramPanel = memo(function UsageHistogramPanel({
  histogram,
  metric,
  onMetricChange,
}: {
  histogram: UsageHistogram | null;
  metric: HistogramMetric;
  onMetricChange: (metric: HistogramMetric) => void;
}) {
  const { t } = useLanguage();
  const formatNumber = useFormatNumber();
  const buckets = histogram?.buckets ?? [];
  const maxValue = Math.max(
    1,
    ...buckets.map((bucket) =>
      metric === "tokens" ? bucket.combinedTotalTokens : bucket.combinedRequestCount,
    ),
  );
  const handleWheel = useCallback((event: React.WheelEvent<HTMLDivElement>) => {
    const element = event.currentTarget;
    if (element.scrollWidth <= element.clientWidth) return;
    if (Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;

    event.preventDefault();
    element.scrollLeft += event.deltaY;
  }, []);

  return (
    <div className="card wide histogram-card">
      <div className="card-header">
        <h2>{t("histogram.title")}</h2>
        <div className="segmented-control">
          {(["tokens", "requests"] as const).map((item) => (
            <button
              key={item}
              type="button"
              className={metric === item ? "active" : ""}
              onClick={() => onMetricChange(item)}
            >
              {t(`histogram.${item}`)}
            </button>
          ))}
        </div>
      </div>
      {buckets.length ? (
        <div
          className={"histogram" + (buckets.length <= 10 ? " fit" : "")}
          onWheel={handleWheel}
        >
          {buckets.map((bucket) => {
            const value =
              metric === "tokens" ? bucket.combinedTotalTokens : bucket.combinedRequestCount;
            return (
              <div className="histogram-bar" key={bucket.bucket}>
                <div className="histogram-track">
                  <span style={{ height: `${Math.max(4, (value / maxValue) * 100)}%` }} />
                </div>
                <strong>{formatNumber(value)}</strong>
                <small>{formatHistogramLabel(bucket.label, histogram?.granularity)}</small>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="recent-table-state">
          <p className="empty">{t("histogram.empty")}</p>
        </div>
      )}
    </div>
  );
});

function formatHistogramLabel(label: string, granularity: string | undefined) {
  if (granularity === "hour") return label.slice(11, 16);
  return label.slice(5);
}

const RecentRequestsPanel = memo(function RecentRequestsPanel({
  provider,
  title,
  dataDir,
  emptyText,
  ready,
  refreshToken,
}: {
  provider: "codex" | "claude_code";
  title: string;
  dataDir: string | undefined;
  emptyText: string;
  ready: boolean;
  refreshToken: number;
}) {
  const { t } = useLanguage();
  const formatNumber = useFormatNumber();
  const tableContainerRef = useRef<HTMLDivElement | null>(null);
  const [pageIndex, setPageIndex] = useState(0);
  const [records, setRecords] = useState<RequestRecordListItem[]>([]);
  const [total, setTotal] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [panelError, setPanelError] = useState<string | null>(null);

  useEffect(() => {
    setPageIndex(0);
  }, [provider]);

  useEffect(() => {
    if (!ready) return;

    let disposed = false;
    setIsLoading(true);
    setPanelError(null);

    void listFilteredRequests({
      provider,
      limit: OVERVIEW_RECENT_PAGE_SIZE,
      offset: pageIndex * OVERVIEW_RECENT_PAGE_SIZE,
    })
      .then((result) => {
        if (disposed) return;
        setRecords(result.records);
        setTotal(result.total);
      })
      .catch(() => {
        if (disposed) return;
        setPanelError(t("error.loadRequests"));
      })
      .finally(() => {
        if (!disposed) {
          setIsLoading(false);
        }
      });

    return () => {
      disposed = true;
    };
  }, [pageIndex, provider, ready, refreshToken, t]);

  const start = total === 0 ? 0 : pageIndex * OVERVIEW_RECENT_PAGE_SIZE + 1;
  const end = total === 0 ? 0 : pageIndex * OVERVIEW_RECENT_PAGE_SIZE + records.length;
  const hasPreviousPage = pageIndex > 0;
  const hasNextPage = (pageIndex + 1) * OVERVIEW_RECENT_PAGE_SIZE < total;
  const hasRecords = records.length > 0;
  const showLoadingState = !ready || (!hasRecords && isLoading);
  const changePage = (nextPageIndex: number) => {
    tableContainerRef.current?.scrollTo({ top: 0, left: 0, behavior: "auto" });
    setPageIndex(nextPageIndex);
  };

  return (
    <div className="card wide recent-requests-card">
      <div className="card-header">
        <h2>{title}</h2>
        <span className="card-meta mono recent-card-meta">{dataDir ?? t("recent.resolving")}</span>
      </div>
      {panelError ? <div className="notice error">{panelError}</div> : null}
      {showLoadingState ? (
        <div className="recent-table-state">{t("loading.requests")}</div>
      ) : hasRecords ? (
        <div
          ref={tableContainerRef}
          className={"table-container recent-table-container" + (isLoading ? " refreshing" : "")}
        >
          <table className="data-table">
            <thead>
              <tr>
                <th>{t("th.request")}</th>
                <th>{t("th.mode")}</th>
                <th>{t("th.input")}</th>
                <th>{t("th.output")}</th>
                <th>{t("th.cached")}</th>
                <th>{t("th.reasoning")}</th>
                <th>{t("th.ttft")}</th>
                <th>{t("th.duration")}</th>
                <th>{t("th.status")}</th>
                <th>{t("th.started")}</th>
              </tr>
            </thead>
            <tbody>
              {records.map((request) => (
                <tr key={request.id} className="request-row">
                  <td>
                    <div className="primary-cell">
                      <strong>{request.model ?? t("model.unknown")}</strong>
                      <span>{request.requestId ?? request.id}</span>
                    </div>
                  </td>
                  <td>{request.isStream ? t("mode.stream") : t("mode.nonStream")}</td>
                  <td>{formatNumber(request.inputTokens)}</td>
                  <td>{formatNumber(request.outputTokens)}</td>
                  <td>{formatNumber(request.cachedInputTokens)}</td>
                  <td>{formatNumber(request.reasoningTokens)}</td>
                  <td>{formatDuration(request.ttftMs, t)}</td>
                  <td>{formatDuration(request.durationMs, t)}</td>
                  <td>{request.status}</td>
                  <td>{formatDate(request.startedAt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {isLoading ? <div className="table-refresh-indicator">{t("loading.requests")}</div> : null}
        </div>
      ) : (
        <div className="recent-table-state">
          <p className="empty">{emptyText}</p>
        </div>
      )}
      <div className="recent-pagination">
        <button
          type="button"
          className="ghost"
          onClick={() => changePage(pageIndex - 1)}
          disabled={showLoadingState || !hasPreviousPage}
        >
          {t("pagination.previous")}
        </button>
        <span className="pagination-info">
          {t("pagination.info", formatNumber(start), formatNumber(end), formatNumber(total))}
        </span>
        <button
          type="button"
          className="ghost"
          onClick={() => changePage(pageIndex + 1)}
          disabled={showLoadingState || !hasNextPage}
        >
          {t("pagination.next")}
        </button>
      </div>
    </div>
  );
});

function formatDuration(value: number | null | undefined, t: (k: string) => string) {
  if (value == null) return t("n/a");
  if (value >= 1000) return `${(value / 1000).toFixed(2)} s`;
  return `${value} ms`;
}

function formatDate(value: string | null | undefined) {
  if (!value) return "N/A";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;

  return new Intl.DateTimeFormat(undefined, {
    timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function App() {
  const { t } = useLanguage();
  const [databaseSummary, setDatabaseSummary] = useState<DatabaseSummary | null>(null);
  const [codexOverview, setCodexOverview] = useState<CodexOverview | null>(null);
  const [claudeOverview, setClaudeOverview] = useState<ClaudeOverview | null>(null);
  const [period, setPeriod] = useState<Period>("today");
  const [periodUsage, setPeriodUsage] = useState<Record<Period, CombinedUsage | null>>({
    today: null,
    week: null,
    month: null,
    total: null,
  });
  const [histograms, setHistograms] = useState<Record<"today" | "week" | "month", UsageHistogram | null>>({
    today: null,
    week: null,
    month: null,
  });
  const [histogramMetric, setHistogramMetric] = useState<HistogramMetric>("tokens");
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const [currentPage, setCurrentPage] = useState<"overview" | "requests" | "settings">("overview");
  const [visitedRequests, setVisitedRequests] = useState(false);
  const [visitedSettings, setVisitedSettings] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [overviewRefreshToken, setOverviewRefreshToken] = useState(0);

  const fetchAllPeriodUsage = async () => {
    const periods: Period[] = ["today", "week", "month", "total"];
    const results = await Promise.all(
      periods.map(async (p) => {
        try {
          const usage = p === "total" ? await getCombinedUsageTotal() : await getCombinedUsage(getDateRangeForPeriod(p));
          return [p, usage] as const;
        } catch {
          return [p, null] as const;
        }
      }),
    );
    setPeriodUsage(Object.fromEntries(results) as Record<Period, CombinedUsage | null>);
  };

  const fetchAllHistograms = async () => {
    const periods: Array<"today" | "week" | "month"> = ["today", "week", "month"];
    const results = await Promise.all(
      periods.map(async (p) => {
        try {
          const histogram = await getUsageHistogram({
            period: p,
            granularity: p === "today" ? "hour" : "day",
          });
          return [p, histogram] as const;
        } catch {
          return [p, null] as const;
        }
      }),
    );
    setHistograms(Object.fromEntries(results) as Record<"today" | "week" | "month", UsageHistogram | null>);
  };

  const fetchPeriodUsage = async (p: Period) => {
    try {
      const usage = p === "total" ? await getCombinedUsageTotal() : await getCombinedUsage(getDateRangeForPeriod(p));
      setPeriodUsage((prev) => ({ ...prev, [p]: usage }));
    } catch {
      // period fetch errors are non-critical
    }
  };

  const refresh = () => {
    startTransition(async () => {
      try {
        setError(null);
        const [summary, codex, claude] = await Promise.all([
          getDatabaseSummary(),
          getCodexOverview(),
          getClaudeCodeOverview(),
        ]);
        setDatabaseSummary(summary);
        setCodexOverview(codex);
        setClaudeOverview(claude);
        setOverviewRefreshToken((current) => current + 1);
      } catch (refreshError) {
        setError(
          refreshError instanceof Error ? refreshError.message : t("error.loadAppState"),
        );
      }
    });
  };

  useEffect(() => {
    refresh();
    fetchAllPeriodUsage();
    fetchAllHistograms();
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlistenCompleted: (() => void) | null = null;
    let unlistenFailed: (() => void) | null = null;

    void Promise.all([
      listen("usage-sync-completed", () => {
        refresh();
        fetchAllPeriodUsage();
        fetchAllHistograms();
      }),
      listen<{ error?: string }>("usage-sync-failed", (event) => {
        setError(event.payload?.error ?? t("error.syncFailed"));
      }),
    ]).then(([completed, failed]) => {
      if (disposed) {
        completed();
        failed();
        return;
      }
      unlistenCompleted = completed;
      unlistenFailed = failed;
    });

    return () => {
      disposed = true;
      unlistenCompleted?.();
      unlistenFailed?.();
    };
  }, [period]);

  const handlePeriodChange = (p: Period) => {
    setPeriod(p);
    if (!periodUsage[p]) fetchPeriodUsage(p);
  };

  const handleInitializeDatabase = async () => {
    startTransition(async () => {
      try {
        setError(null);
        await initializeLocalDatabase();
        refresh();
        fetchAllPeriodUsage();
        fetchAllHistograms();
      } catch (initError) {
        setError(
          initError instanceof Error ? initError.message : t("error.initDb"),
        );
      }
    });
  };

  const handleNavigate = (page: "overview" | "requests" | "settings") => {
    if (page === "requests") setVisitedRequests(true);
    if (page === "settings") setVisitedSettings(true);
    setCurrentPage(page);
  };

  const navItems = [
    { key: "overview" as const, label: t("nav.overview"), icon: GridIcon },
    { key: "requests" as const, label: t("nav.requests"), icon: ListIcon },
    { key: "settings" as const, label: t("nav.settings"), icon: SettingsIcon },
  ];

  return (
    <div className="app-layout">
      <aside className={"sidebar" + (sidebarCollapsed ? " collapsed" : "")}>
        <div className="sidebar-header">
          <h1>{sidebarCollapsed ? t("app.title.collapsed") : t("app.title")}</h1>
        </div>
        <nav className="sidebar-nav">
          {navItems.map(({ key, label, icon: Icon }) => (
            <button
              key={key}
              className={"nav-item" + (currentPage === key ? " active" : "")}
              onClick={() => handleNavigate(key)}
            >
              <Icon />
              <span>{label}</span>
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          {!sidebarCollapsed && <span className="version">{t("version")}</span>}
          <button
            className="collapse-btn"
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            title={sidebarCollapsed ? t("sidebar.expand") : t("sidebar.collapse")}
          >
            <ChevronLeftIcon />
          </button>
        </div>
      </aside>
      <main className="main-content">
        {currentPage === "overview" && (
          <OverviewPage
            databaseSummary={databaseSummary}
            codexOverview={codexOverview}
            claudeOverview={claudeOverview}
            periodUsage={periodUsage[period]}
            histogram={period === "total" ? null : histograms[period]}
            period={period}
            histogramMetric={histogramMetric}
            refreshToken={overviewRefreshToken}
            error={error}
            isPending={isPending}
            onPeriodChange={handlePeriodChange}
            onHistogramMetricChange={setHistogramMetric}
            onRefresh={refresh}
            onInitializeDatabase={handleInitializeDatabase}
          />
        )}
        {currentPage === "requests" && visitedRequests && (
          <Suspense fallback={<div className="loading">{t("loading.requests")}</div>}>
            <Requests />
          </Suspense>
        )}
        {currentPage === "settings" && visitedSettings && (
          <Suspense fallback={<div className="loading">{t("loading.settings")}</div>}>
            <Settings />
          </Suspense>
        )}
      </main>
    </div>
  );
}

export default App;
