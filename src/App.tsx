import { useEffect, useState, useTransition } from "react";
import {
  getClaudeCodeOverview,
  getCodexOverview,
  getDatabaseSummary,
  initializeLocalDatabase,
  syncClaudeCodeSessions,
  syncCodexSessions,
  type ClaudeCodeSyncSummary,
  type ClaudeOverview,
  type CodexOverview,
  type CodexSyncSummary,
  type DailyUsageRecord,
  type DatabaseSummary,
  type RequestRecordListItem,
} from "./desktop";
import Requests from "./Requests";
import Settings from "./Settings";
import "./App.css";

function formatNumber(value: number | null | undefined) {
  if (value == null) {
    return "0";
  }

  return new Intl.NumberFormat("en-US").format(value);
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

  return date.toLocaleString("zh-CN", {
    hour12: false,
  });
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

function App() {
  const [databaseSummary, setDatabaseSummary] = useState<DatabaseSummary | null>(null);
  const [codexOverview, setCodexOverview] = useState<CodexOverview | null>(null);
  const [claudeOverview, setClaudeOverview] = useState<ClaudeOverview | null>(null);
  const [lastCodexSync, setLastCodexSync] = useState<CodexSyncSummary | null>(null);
  const [lastClaudeSync, setLastClaudeSync] = useState<ClaudeCodeSyncSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const [currentPage, setCurrentPage] = useState<"overview" | "requests" | "settings">("overview");

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

  const handleSyncCodex = async () => {
    startTransition(async () => {
      try {
        setError(null);
        const syncSummary = await syncCodexSessions();
        setLastCodexSync(syncSummary);
        const [summary, overview] = await Promise.all([getDatabaseSummary(), getCodexOverview()]);
        setDatabaseSummary(summary);
        setCodexOverview(overview);
      } catch (syncError) {
        setError(syncError instanceof Error ? syncError.message : "Failed to sync Codex data.");
      }
    });
  };

  const handleSyncClaude = async () => {
    startTransition(async () => {
      try {
        setError(null);
        const syncSummary = await syncClaudeCodeSessions();
        setLastClaudeSync(syncSummary);
        const [summary, overview] = await Promise.all([
          getDatabaseSummary(),
          getClaudeCodeOverview(),
        ]);
        setDatabaseSummary(summary);
        setClaudeOverview(overview);
      } catch (syncError) {
        setError(
          syncError instanceof Error ? syncError.message : "Failed to sync Claude Code data.",
        );
      }
    });
  };

  const codexTodayUsage: DailyUsageRecord | null =
    codexOverview?.todayUsage ?? lastCodexSync?.todayUsage ?? null;
  const claudeTodayUsage: DailyUsageRecord | null =
    claudeOverview?.todayUsage ?? lastClaudeSync?.todayUsage ?? null;

  if (currentPage === "requests") {
    return (
      <div className="app-shell">
        <nav className="top-nav">
          <button type="button" className="secondary" onClick={() => setCurrentPage("overview")}>
            Back
          </button>
          <h2>Request Records</h2>
        </nav>
        <Requests />
      </div>
    );
  }

  if (currentPage === "settings") {
    return (
      <div className="app-shell">
        <nav className="top-nav">
          <button type="button" className="secondary" onClick={() => setCurrentPage("overview")}>
            Back
          </button>
          <h2>Settings</h2>
        </nav>
        <Settings />
      </div>
    );
  }

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
          <button type="button" onClick={handleSyncCodex} disabled={isPending}>
            Sync Codex
          </button>
          <button type="button" onClick={handleSyncClaude} disabled={isPending}>
            Sync Claude
          </button>
          <button type="button" className="secondary" onClick={() => setCurrentPage("requests")}>
            Requests
          </button>
          <button type="button" className="secondary" onClick={() => setCurrentPage("settings")}>
            Settings
          </button>
          <button type="button" className="secondary" onClick={refresh} disabled={isPending}>
            Refresh
          </button>
          <button
            type="button"
            className="secondary"
            onClick={handleInitializeDatabase}
            disabled={isPending}
          >
            Init DB
          </button>
        </div>
      </section>

      {error ? <section className="notice error">{error}</section> : null}

      {lastCodexSync ? (
        <section className="notice">
          Codex imported {formatNumber(lastCodexSync.importedRequests)} requests from{" "}
          {formatNumber(lastCodexSync.scannedFiles)} files.
        </section>
      ) : null}

      {lastClaudeSync ? (
        <section className="notice">
          Claude imported {formatNumber(lastClaudeSync.importedRequests)} requests from{" "}
          {formatNumber(lastClaudeSync.scannedFiles)} files.
        </section>
      ) : null}

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

        <article className="panel wide">
          <div className="panel-header">
            <h2>Recent Codex Requests</h2>
            <span className="panel-meta mono">{codexOverview?.dataDir ?? "resolving"}</span>
          </div>
          {codexOverview?.recentRequests.length ? (
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
                <tbody>{codexOverview.recentRequests.map(renderRequestRow)}</tbody>
              </table>
            </div>
          ) : (
            <p className="empty">No Codex requests yet.</p>
          )}
        </article>

        <article className="panel wide">
          <div className="panel-header">
            <h2>Recent Claude Requests</h2>
            <span className="panel-meta mono">{claudeOverview?.dataDir ?? "resolving"}</span>
          </div>
          {claudeOverview?.recentRequests.length ? (
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
                <tbody>{claudeOverview.recentRequests.map(renderRequestRow)}</tbody>
              </table>
            </div>
          ) : (
            <p className="empty">No Claude requests yet.</p>
          )}
        </article>

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
}

export default App;
