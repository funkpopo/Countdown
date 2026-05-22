import { lazy, memo, Suspense, useEffect, useState, useTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getClaudeCodeOverview,
  getCodexOverview,
  getDatabaseSummary,
  initializeLocalDatabase,
  type ClaudeOverview,
  type CodexOverview,
  type DailyUsageRecord,
  type DatabaseSummary,
  type RequestRecordListItem,
} from "./desktop";
import "./App.css";

const Requests = lazy(() => import("./Requests"));
const Settings = lazy(() => import("./Settings"));
const numberFormatter = new Intl.NumberFormat("en-US");
const dateTimeFormatter = new Intl.DateTimeFormat("zh-CN", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

function formatNumber(value: number | null | undefined) {
  if (value == null) return "0";
  return numberFormatter.format(value);
}

function formatMs(value: number | null | undefined) {
  if (value == null) return "N/A";
  if (value >= 1000) return `${(value / 1000).toFixed(2)} s`;
  return `${value} ms`;
}

function formatDateTime(value: string | null | undefined) {
  if (!value) return "N/A";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return dateTimeFormatter.format(date);
}

function renderUsageStat(label: string, value: string) {
  return (
    <div className="stat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function renderRequestRow(request: RequestRecordListItem) {
  return (
    <tr key={request.id}>
      <td>
        <div className="primary-cell">
          <strong>{request.model ?? "Unknown model"}</strong>
          <span>{request.requestId ?? request.id}</span>
        </div>
      </td>
      <td>{request.isStream ? "Stream" : "Non-stream"}</td>
      <td>{formatNumber(request.inputTokens)}</td>
      <td>{formatNumber(request.outputTokens)}</td>
      <td>{formatNumber(request.cachedInputTokens)}</td>
      <td>{formatNumber(request.reasoningTokens)}</td>
      <td>{formatMs(request.ttftMs)}</td>
      <td>{formatMs(request.durationMs)}</td>
      <td>{request.status}</td>
      <td>{formatDateTime(request.startedAt)}</td>
    </tr>
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

type OverviewPageProps = {
  databaseSummary: DatabaseSummary | null;
  codexOverview: CodexOverview | null;
  claudeOverview: ClaudeOverview | null;
  error: string | null;
  isPending: boolean;
  onRefresh: () => void;
  onInitializeDatabase: () => void;
};

const OverviewPage = memo(function OverviewPage({
  databaseSummary,
  codexOverview,
  claudeOverview,
  error,
  isPending,
  onRefresh,
  onInitializeDatabase,
}: OverviewPageProps) {
  const codexTodayUsage: DailyUsageRecord | null = codexOverview?.todayUsage ?? null;
  const claudeTodayUsage: DailyUsageRecord | null = claudeOverview?.todayUsage ?? null;

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>Overview</h1>
          <p className="page-meta">
            {formatNumber(databaseSummary?.providerProfiles.length)} profiles ·{" "}
            {formatNumber(codexOverview?.requestCount)} Codex requests ·{" "}
            {formatNumber(claudeOverview?.requestCount)} Claude requests
          </p>
        </div>
        <div className="actions">
          <button type="button" onClick={onRefresh} disabled={isPending}>
            Refresh
          </button>
          <button
            type="button"
            className="ghost"
            onClick={onInitializeDatabase}
            disabled={isPending}
          >
            Init DB
          </button>
        </div>
      </div>

      {error ? <div className="notice error">{error}</div> : null}

      <div className="grid">
        <div className="card">
          <div className="card-header">
            <h2>Codex Today</h2>
            <span className="card-meta">{formatNumber(codexOverview?.sessionCount)} sessions</span>
          </div>
          <div className="stats">
            {renderUsageStat("Input", formatNumber(codexTodayUsage?.inputTokens))}
            {renderUsageStat("Output", formatNumber(codexTodayUsage?.outputTokens))}
            {renderUsageStat("Total", formatNumber(codexTodayUsage?.totalTokens))}
            {renderUsageStat("Requests", formatNumber(codexTodayUsage?.requestCount))}
            {renderUsageStat("Avg TTFT", formatMs(codexTodayUsage?.avgTtftMs ?? null))}
            {renderUsageStat("Avg Duration", formatMs(codexTodayUsage?.avgDurationMs ?? null))}
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <h2>Claude Today</h2>
            <span className="card-meta">
              {formatNumber(claudeOverview?.sessionCount)} sessions
            </span>
          </div>
          <div className="stats">
            {renderUsageStat("Input", formatNumber(claudeTodayUsage?.inputTokens))}
            {renderUsageStat("Output", formatNumber(claudeTodayUsage?.outputTokens))}
            {renderUsageStat("Total", formatNumber(claudeTodayUsage?.totalTokens))}
            {renderUsageStat("Requests", formatNumber(claudeTodayUsage?.requestCount))}
            {renderUsageStat("Avg TTFT", formatMs(claudeTodayUsage?.avgTtftMs ?? null))}
            {renderUsageStat("Avg Duration", formatMs(claudeTodayUsage?.avgDurationMs ?? null))}
          </div>
        </div>

        <RecentRequestsPanel
          title="Recent Codex Requests"
          dataDir={codexOverview?.dataDir}
          requests={codexOverview?.recentRequests}
          emptyText="No Codex requests yet."
        />

        <RecentRequestsPanel
          title="Recent Claude Requests"
          dataDir={claudeOverview?.dataDir}
          requests={claudeOverview?.recentRequests}
          emptyText="No Claude requests yet."
        />

        <div className="card wide">
          <div className="card-header">
            <h2>Storage</h2>
            <span className="card-meta">{formatNumber(databaseSummary?.tables.length)} tables</span>
          </div>
          <div className="table-grid">
            {databaseSummary?.tables.map((table) => (
              <div key={table.tableName} className="table-card">
                <strong>{table.tableName}</strong>
                <span>{formatNumber(table.rowCount)} rows</span>
              </div>
            )) ?? <p className="empty">Waiting for schema summary...</p>}
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
                <th>Request</th>
                <th>Mode</th>
                <th>Input</th>
                <th>Output</th>
                <th>Cached</th>
                <th>Reasoning</th>
                <th>TTFT</th>
                <th>Duration</th>
                <th>Status</th>
                <th>Started</th>
              </tr>
            </thead>
            <tbody>{requests.map(renderRequestRow)}</tbody>
          </table>
        </div>
      ) : (
        <p className="empty">{emptyText}</p>
      )}
    </div>
  );
});

const NAV_ITEMS = [
  { key: "overview" as const, label: "Overview", icon: GridIcon },
  { key: "requests" as const, label: "Requests", icon: ListIcon },
  { key: "settings" as const, label: "Settings", icon: SettingsIcon },
];

function App() {
  const [databaseSummary, setDatabaseSummary] = useState<DatabaseSummary | null>(null);
  const [codexOverview, setCodexOverview] = useState<CodexOverview | null>(null);
  const [claudeOverview, setClaudeOverview] = useState<ClaudeOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const [currentPage, setCurrentPage] = useState<"overview" | "requests" | "settings">("overview");
  const [visitedRequests, setVisitedRequests] = useState(false);
  const [visitedSettings, setVisitedSettings] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

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
          refreshError instanceof Error ? refreshError.message : "Failed to load app state.",
        );
      }
    });
  };

  useEffect(() => {
    refresh();
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlistenCompleted: (() => void) | null = null;
    let unlistenFailed: (() => void) | null = null;

    void Promise.all([
      listen("usage-sync-completed", () => {
        refresh();
      }),
      listen<{ error?: string }>("usage-sync-failed", (event) => {
        setError(event.payload?.error ?? "Background sync failed.");
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
  }, []);

  const handleInitializeDatabase = async () => {
    startTransition(async () => {
      try {
        setError(null);
        await initializeLocalDatabase();
        refresh();
      } catch (initError) {
        setError(
          initError instanceof Error ? initError.message : "Failed to initialize database.",
        );
      }
    });
  };

  const handleNavigate = (page: "overview" | "requests" | "settings") => {
    if (page === "requests") setVisitedRequests(true);
    if (page === "settings") setVisitedSettings(true);
    setCurrentPage(page);
  };

  return (
    <div className="app-layout">
      <aside className={"sidebar" + (sidebarCollapsed ? " collapsed" : "")}>
        <div className="sidebar-header">
          <h1>{sidebarCollapsed ? "C" : "Countdown"}</h1>
        </div>
        <nav className="sidebar-nav">
          {NAV_ITEMS.map(({ key, label, icon: Icon }) => (
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
          {!sidebarCollapsed && <span className="version">v0.1.0</span>}
          <button
            className="collapse-btn"
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
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
            error={error}
            isPending={isPending}
            onRefresh={refresh}
            onInitializeDatabase={handleInitializeDatabase}
          />
        )}
        {currentPage === "requests" && visitedRequests && (
          <Suspense fallback={<div className="loading">Loading requests...</div>}>
            <Requests />
          </Suspense>
        )}
        {currentPage === "settings" && visitedSettings && (
          <Suspense fallback={<div className="loading">Loading settings...</div>}>
            <Settings />
          </Suspense>
        )}
      </main>
    </div>
  );
}

export default App;
