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
  if (value == null) {
    return "0";
  }

  return numberFormatter.format(value);
}

function formatMs(value: number | null | undefined) {
  if (value == null) {
    return "N/A";
  }

  if (value >= 1000) {
    return `${(value / 1000).toFixed(2)} s`;
  }

  return `${value} ms`;
}

function formatDateTime(value: string | null | undefined) {
  if (!value) {
    return "N/A";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return dateTimeFormatter.format(date);
}

function renderUsageStat(label: string, value: string) {
  return (
    <div className="stat-card">
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

type OverviewPageProps = {
  databaseSummary: DatabaseSummary | null;
  codexOverview: CodexOverview | null;
  claudeOverview: ClaudeOverview | null;
  error: string | null;
  isPending: boolean;
  onShowRequests: () => void;
  onShowSettings: () => void;
  onRefresh: () => void;
  onInitializeDatabase: () => void;
};

const OverviewPage = memo(function OverviewPage({
  databaseSummary,
  codexOverview,
  claudeOverview,
  error,
  isPending,
  onShowRequests,
  onShowSettings,
  onRefresh,
  onInitializeDatabase,
}: OverviewPageProps) {
  const codexTodayUsage: DailyUsageRecord | null = codexOverview?.todayUsage ?? null;
  const claudeTodayUsage: DailyUsageRecord | null = claudeOverview?.todayUsage ?? null;

  return (
    <main className="shell">
      <section className="workspace-header">
        <div>
          <h1>Countdown</h1>
          <p className="workspace-meta">
            {formatNumber(databaseSummary?.providerProfiles.length)} profiles ·{" "}
            {formatNumber(codexOverview?.requestCount)} Codex requests ·{" "}
            {formatNumber(claudeOverview?.requestCount)} Claude requests
          </p>
        </div>

        <div className="toolbar">
          <button type="button" className="secondary" onClick={onShowRequests}>
            Requests
          </button>
          <button type="button" className="secondary" onClick={onShowSettings}>
            Settings
          </button>
          <button type="button" className="secondary" onClick={onRefresh} disabled={isPending}>
            Refresh
          </button>
          <button
            type="button"
            className="secondary"
            onClick={onInitializeDatabase}
            disabled={isPending}
          >
            Init DB
          </button>
        </div>
      </section>

      {error ? <section className="notice error">{error}</section> : null}

      <section className="grid">
        <article className="panel">
          <div className="panel-header">
            <h2>Codex Today</h2>
            <span className="panel-meta">{formatNumber(codexOverview?.sessionCount)} sessions</span>
          </div>
          <div className="stats-grid">
            {renderUsageStat("Input", formatNumber(codexTodayUsage?.inputTokens))}
            {renderUsageStat("Output", formatNumber(codexTodayUsage?.outputTokens))}
            {renderUsageStat("Total", formatNumber(codexTodayUsage?.totalTokens))}
            {renderUsageStat("Requests", formatNumber(codexTodayUsage?.requestCount))}
            {renderUsageStat("Avg TTFT", formatMs(codexTodayUsage?.avgTtftMs ?? null))}
            {renderUsageStat("Avg Duration", formatMs(codexTodayUsage?.avgDurationMs ?? null))}
          </div>
        </article>

        <article className="panel">
          <div className="panel-header">
            <h2>Claude Today</h2>
            <span className="panel-meta">
              {formatNumber(claudeOverview?.sessionCount)} sessions
            </span>
          </div>
          <div className="stats-grid">
            {renderUsageStat("Input", formatNumber(claudeTodayUsage?.inputTokens))}
            {renderUsageStat("Output", formatNumber(claudeTodayUsage?.outputTokens))}
            {renderUsageStat("Total", formatNumber(claudeTodayUsage?.totalTokens))}
            {renderUsageStat("Requests", formatNumber(claudeTodayUsage?.requestCount))}
            {renderUsageStat("Avg TTFT", formatMs(claudeTodayUsage?.avgTtftMs ?? null))}
            {renderUsageStat("Avg Duration", formatMs(claudeTodayUsage?.avgDurationMs ?? null))}
          </div>
        </article>

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

        <article className="panel wide">
          <div className="panel-header">
            <h2>Storage</h2>
            <span className="panel-meta">{formatNumber(databaseSummary?.tables.length)} tables</span>
          </div>
          <div className="table-grid">
            {databaseSummary?.tables.map((table) => (
              <div key={table.tableName} className="table-card">
                <strong>{table.tableName}</strong>
                <span>{formatNumber(table.rowCount)} rows</span>
              </div>
            )) ?? <p className="empty">Waiting for schema summary...</p>}
          </div>
        </article>
      </section>
    </main>
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
    <article className="panel wide">
      <div className="panel-header">
        <h2>{title}</h2>
        <span className="panel-meta mono">{dataDir ?? "resolving"}</span>
      </div>
      {requests?.length ? (
        <div className="table-shell">
          <table className="request-table">
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
    </article>
  );
});

function App() {
  const [databaseSummary, setDatabaseSummary] = useState<DatabaseSummary | null>(null);
  const [codexOverview, setCodexOverview] = useState<CodexOverview | null>(null);
  const [claudeOverview, setClaudeOverview] = useState<ClaudeOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const [currentPage, setCurrentPage] = useState<"overview" | "requests" | "settings">("overview");
  const [visitedRequests, setVisitedRequests] = useState(false);
  const [visitedSettings, setVisitedSettings] = useState(false);

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

  return (
    <>
      <div hidden={currentPage !== "overview"}>
        <OverviewPage
          databaseSummary={databaseSummary}
          codexOverview={codexOverview}
          claudeOverview={claudeOverview}
          error={error}
          isPending={isPending}
          onShowRequests={() => {
            setVisitedRequests(true);
            setCurrentPage("requests");
          }}
          onShowSettings={() => {
            setVisitedSettings(true);
            setCurrentPage("settings");
          }}
          onRefresh={refresh}
          onInitializeDatabase={handleInitializeDatabase}
        />
      </div>

      {visitedRequests ? (
        <div hidden={currentPage !== "requests"} className="app-shell">
          <nav className="top-nav">
            <button type="button" className="secondary" onClick={() => setCurrentPage("overview")}>
              Back
            </button>
            <h2>Request Records</h2>
          </nav>
          <Suspense fallback={<section className="notice">Loading requests...</section>}>
            <Requests />
          </Suspense>
        </div>
      ) : null}

      {visitedSettings ? (
        <div hidden={currentPage !== "settings"} className="app-shell">
          <nav className="top-nav">
            <button type="button" className="secondary" onClick={() => setCurrentPage("overview")}>
              Back
            </button>
            <h2>Settings</h2>
          </nav>
          <Suspense fallback={<section className="notice">Loading settings...</section>}>
            <Settings />
          </Suspense>
        </div>
      ) : null}
    </>
  );
}

export default App;
