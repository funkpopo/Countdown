import { lazy, memo, Suspense, useCallback, useEffect, useMemo, useState, useTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getClaudeCodeOverview,
  getCodexOverview,
  getCombinedUsage,
  getDatabaseSummary,
  initializeLocalDatabase,
  type ClaudeOverview,
  type CodexOverview,
  type CombinedUsage,
  type DailyUsageRecord,
  type DatabaseSummary,
  type RequestRecordListItem,
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

type Period = "today" | "week" | "month";

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
  period: Period;
  error: string | null;
  isPending: boolean;
  onPeriodChange: (period: Period) => void;
  onRefresh: () => void;
  onInitializeDatabase: () => void;
};

const OverviewPage = memo(function OverviewPage({
  databaseSummary,
  codexOverview,
  claudeOverview,
  periodUsage,
  period,
  error,
  isPending,
  onPeriodChange,
  onRefresh,
  onInitializeDatabase,
}: OverviewPageProps) {
  const { t } = useLanguage();
  const formatNumber = useFormatNumber();
  const formatMs = useFormatMs();

  const codexTodayUsage: DailyUsageRecord | null = codexOverview?.todayUsage ?? null;
  const claudeTodayUsage: DailyUsageRecord | null = claudeOverview?.todayUsage ?? null;

  return (
    <div className="page">
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
        {(["today", "week", "month"] as const).map((p) => (
          <button
            key={p}
            className={"period-tab" + (period === p ? " active" : "")}
            onClick={() => onPeriodChange(p)}
          >
            {t(`tab.${p}`)}
          </button>
        ))}
        <span className="period-range">
          {periodUsage
            ? `${periodUsage.startDate} ~ ${periodUsage.endDate}`
            : "..."}
        </span>
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

        <RecentRequestsPanel
          title={t("recent.codex")}
          dataDir={codexOverview?.dataDir}
          requests={codexOverview?.recentRequests}
          emptyText={t("recent.empty.codex")}
        />

        <RecentRequestsPanel
          title={t("recent.claude")}
          dataDir={claudeOverview?.dataDir}
          requests={claudeOverview?.recentRequests}
          emptyText={t("recent.empty.claude")}
        />

        <div className="card wide">
          <div className="card-header">
            <h2>{t("storage.title")}</h2>
            <span className="card-meta">{t("storage.tables", formatNumber(databaseSummary?.tables.length))}</span>
          </div>
          <div className="table-grid">
            {databaseSummary?.tables.map((table) => (
              <div key={table.tableName} className="table-card">
                <strong>{table.tableName}</strong>
                <span>{formatNumber(table.rowCount)} {t("storage.rows")}</span>
              </div>
            )) ?? <p className="empty">{t("storage.waiting")}</p>}
          </div>
        </div>
      </div>
    </div>
  );
});

const RecentRequestsPanel = memo(function RecentRequestsPanel({
  title,
  dataDir,
  requests,
  emptyText,
}: {
  title: string;
  dataDir: string | undefined;
  requests: RequestRecordListItem[] | undefined;
  emptyText: string;
}) {
  const { t } = useLanguage();

  return (
    <div className="card wide">
      <div className="card-header">
        <h2>{title}</h2>
        <span className="card-meta mono">{dataDir ?? "resolving"}</span>
      </div>
      {requests?.length ? (
        <div className="table-container">
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
              {requests.map((request) => (
                <tr key={request.id} className="request-row">
                  <td>
                    <div className="primary-cell">
                      <strong>{request.model ?? t("model.unknown")}</strong>
                      <span>{request.requestId ?? request.id}</span>
                    </div>
                  </td>
                  <td>{request.isStream ? t("mode.stream") : t("mode.nonStream")}</td>
                  <td>{request.inputTokens ?? "0"}</td>
                  <td>{request.outputTokens ?? "0"}</td>
                  <td>{request.cachedInputTokens ?? "0"}</td>
                  <td>{request.reasoningTokens ?? "0"}</td>
                  <td>{formatDuration(request.ttftMs, t)}</td>
                  <td>{formatDuration(request.durationMs, t)}</td>
                  <td>{request.status}</td>
                  <td>{formatDate(request.startedAt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="empty">{emptyText}</p>
      )}
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
  return value;
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
  });
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const [currentPage, setCurrentPage] = useState<"overview" | "requests" | "settings">("overview");
  const [visitedRequests, setVisitedRequests] = useState(false);
  const [visitedSettings, setVisitedSettings] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const fetchAllPeriodUsage = async () => {
    const periods: Period[] = ["today", "week", "month"];
    const results = await Promise.all(
      periods.map(async (p) => {
        try {
          const usage = await getCombinedUsage(getDateRangeForPeriod(p));
          return [p, usage] as const;
        } catch {
          return [p, null] as const;
        }
      }),
    );
    setPeriodUsage(Object.fromEntries(results) as Record<Period, CombinedUsage | null>);
  };

  const fetchPeriodUsage = async (p: Period) => {
    try {
      const usage = await getCombinedUsage(getDateRangeForPeriod(p));
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
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlistenCompleted: (() => void) | null = null;
    let unlistenFailed: (() => void) | null = null;

    void Promise.all([
      listen("usage-sync-completed", () => {
        refresh();
        fetchAllPeriodUsage();
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
            period={period}
            error={error}
            isPending={isPending}
            onPeriodChange={handlePeriodChange}
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
