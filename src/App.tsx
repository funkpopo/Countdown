import { useEffect, useState, useTransition } from "react";
import {
  databaseHealthcheck,
  getBootstrapInfo,
  getClaudeCodeOverview,
  getCodexOverview,
  getDatabaseSummary,
  initializeLocalDatabase,
  syncClaudeCodeSessions,
  syncCodexSessions,
  type BootstrapInfo,
  type ClaudeCodeSyncSummary,
  type ClaudeOverview,
  type CodexOverview,
  type CodexSyncSummary,
  type DatabaseHealth,
  type DatabaseSummary,
  type DailyUsageRecord,
  type RequestRecordListItem,
} from "./desktop";
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
  const [bootstrapInfo, setBootstrapInfo] = useState<BootstrapInfo | null>(null);
  const [databaseHealth, setDatabaseHealth] = useState<DatabaseHealth | null>(null);
  const [databaseSummary, setDatabaseSummary] = useState<DatabaseSummary | null>(null);
  const [codexOverview, setCodexOverview] = useState<CodexOverview | null>(null);
  const [claudeOverview, setClaudeOverview] = useState<ClaudeOverview | null>(null);
  const [lastCodexSync, setLastCodexSync] = useState<CodexSyncSummary | null>(null);
  const [lastClaudeSync, setLastClaudeSync] = useState<ClaudeCodeSyncSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  const refresh = () => {
    startTransition(async () => {
      try {
        setError(null);
        const [bootstrap, health, summary, codex, claude] = await Promise.all([
          getBootstrapInfo(),
          databaseHealthcheck(),
          getDatabaseSummary(),
          getCodexOverview(),
          getClaudeCodeOverview(),
        ]);
        setBootstrapInfo(bootstrap);
        setDatabaseHealth(health);
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
        const [summary, overview] = await Promise.all([
          getDatabaseSummary(),
          getCodexOverview(),
        ]);
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
        setError(syncError instanceof Error ? syncError.message : "Failed to sync Claude Code data.");
      }
    });
  };

  const codexTodayUsage: DailyUsageRecord | null = codexOverview?.todayUsage ?? lastCodexSync?.todayUsage ?? null;
  const claudeTodayUsage: DailyUsageRecord | null = claudeOverview?.todayUsage ?? lastClaudeSync?.todayUsage ?? null;

  return (
    <main className="shell">
      <section className="hero">
        <div>
          <p className="eyebrow">Phase 4 / Claude Code Collector</p>
          <h1>Countdown Desktop</h1>
          <p className="lede">
            Passive Claude Code session ingest from local project JSONL files. The app now imports
            Claude Code history alongside Codex data, normalizes token usage, and exposes request
            records without proxying traffic.
          </p>
        </div>

        <div className="actions">
          <button type="button" onClick={refresh} disabled={isPending}>
            Refresh State
          </button>
          <button type="button" onClick={handleSyncCodex} disabled={isPending}>
            Sync Codex Sessions
          </button>
          <button type="button" onClick={handleSyncClaude} disabled={isPending}>
            Sync Claude Code Sessions
          </button>
          <button
            type="button"
            className="secondary"
            onClick={handleInitializeDatabase}
            disabled={isPending}
          >
            Initialize SQLite
          </button>
        </div>
      </section>

      {error ? <section className="notice error">{error}</section> : null}

      {lastCodexSync ? (
        <section className="notice">
          Synced {formatNumber(lastCodexSync.importedRequests)} Codex requests from{" "}
          {formatNumber(lastCodexSync.scannedFiles)} rollout files. Skipped{" "}
          {formatNumber(lastCodexSync.skippedIncompleteTurns)} incomplete turns.
        </section>
      ) : null}

      {lastClaudeSync ? (
        <section className="notice">
          Synced {formatNumber(lastClaudeSync.importedRequests)} Claude Code requests from{" "}
          {formatNumber(lastClaudeSync.scannedFiles)} project files. Skipped{" "}
          {formatNumber(lastClaudeSync.skippedIncompleteSessions)} incomplete sessions.
        </section>
      ) : null}

      <section className="grid">
        <article className="panel">
          <h2>Today&apos;s Codex Usage</h2>
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
          <h2>Today&apos;s Claude Code Usage</h2>
          <div className="stats-grid">
            {renderUsageStat("Input", formatNumber(claudeTodayUsage?.inputTokens))}
            {renderUsageStat("Output", formatNumber(claudeTodayUsage?.outputTokens))}
            {renderUsageStat("Total", formatNumber(claudeTodayUsage?.totalTokens))}
            {renderUsageStat("Requests", formatNumber(claudeTodayUsage?.requestCount))}
            {renderUsageStat("Avg TTFT", formatMs(claudeTodayUsage?.avgTtftMs ?? null))}
            {renderUsageStat("Avg Duration", formatMs(claudeTodayUsage?.avgDurationMs ?? null))}
          </div>
        </article>

        <article className="panel">
          <h2>Phase Status</h2>
          <dl className="facts">
            <div>
              <dt>Phase 0</dt>
              <dd>{bootstrapInfo?.phase0Complete ? "Completed" : "Pending"}</dd>
            </div>
            <div>
              <dt>Phase 1</dt>
              <dd>{bootstrapInfo?.phase1Complete ? "Completed" : "Pending"}</dd>
            </div>
            <div>
              <dt>Phase 2</dt>
              <dd>{bootstrapInfo?.phase2Complete ? "Completed" : "Pending"}</dd>
            </div>
            <div>
              <dt>Phase 3</dt>
              <dd>{bootstrapInfo?.phase3Complete ? "Completed" : "In Progress"}</dd>
            </div>
            <div>
              <dt>Phase 4</dt>
              <dd>{bootstrapInfo?.phase4Complete ? "Completed" : "In Progress"}</dd>
            </div>
          </dl>
        </article>

        <article className="panel wide">
          <h2>Codex Data Source</h2>
          <dl className="facts">
            <div>
              <dt>Sessions Dir</dt>
              <dd className="mono">{codexOverview?.dataDir ?? "Resolving..."}</dd>
            </div>
            <div>
              <dt>Dir Exists</dt>
              <dd>{String(codexOverview?.dataDirExists ?? false)}</dd>
            </div>
            <div>
              <dt>Imported Sessions</dt>
              <dd>{formatNumber(codexOverview?.sessionCount)}</dd>
            </div>
            <div>
              <dt>Imported Requests</dt>
              <dd>{formatNumber(codexOverview?.requestCount)}</dd>
            </div>
            <div>
              <dt>Last Sync Files</dt>
              <dd>{formatNumber(lastCodexSync?.scannedFiles)}</dd>
            </div>
            <div>
              <dt>Incomplete Turns</dt>
              <dd>{formatNumber(lastCodexSync?.skippedIncompleteTurns)}</dd>
            </div>
          </dl>
        </article>

        <article className="panel wide">
          <h2>Claude Code Data Source</h2>
          <dl className="facts">
            <div>
              <dt>Data Dir</dt>
              <dd className="mono">{claudeOverview?.dataDir ?? "Resolving..."}</dd>
            </div>
            <div>
              <dt>Dir Exists</dt>
              <dd>{String(claudeOverview?.dataDirExists ?? false)}</dd>
            </div>
            <div>
              <dt>Imported Sessions</dt>
              <dd>{formatNumber(claudeOverview?.sessionCount)}</dd>
            </div>
            <div>
              <dt>Imported Requests</dt>
              <dd>{formatNumber(claudeOverview?.requestCount)}</dd>
            </div>
            <div>
              <dt>Last Sync Files</dt>
              <dd>{formatNumber(lastClaudeSync?.scannedFiles)}</dd>
            </div>
            <div>
              <dt>Incomplete Sessions</dt>
              <dd>{formatNumber(lastClaudeSync?.skippedIncompleteSessions)}</dd>
            </div>
          </dl>
        </article>

        <article className="panel">
          <h2>App Runtime</h2>
          <dl className="facts">
            <div>
              <dt>Product</dt>
              <dd>{bootstrapInfo?.productName ?? "Loading..."}</dd>
            </div>
            <div>
              <dt>Version</dt>
              <dd>{bootstrapInfo?.version ?? "Loading..."}</dd>
            </div>
            <div>
              <dt>Identifier</dt>
              <dd>{bootstrapInfo?.identifier ?? "Loading..."}</dd>
            </div>
            <div>
              <dt>App Data Dir</dt>
              <dd className="mono">{bootstrapInfo?.appDataDir ?? "Loading..."}</dd>
            </div>
          </dl>
        </article>

        <article className="panel">
          <h2>SQLite Health</h2>
          <dl className="facts">
            <div>
              <dt>Database Path</dt>
              <dd className="mono">{databaseHealth?.databasePath ?? "Not resolved yet"}</dd>
            </div>
            <div>
              <dt>Exists</dt>
              <dd>{databaseHealth ? String(databaseHealth.exists) : "Unknown"}</dd>
            </div>
            <div>
              <dt>Writable</dt>
              <dd>{databaseHealth ? String(databaseHealth.writable) : "Unknown"}</dd>
            </div>
            <div>
              <dt>Migrations</dt>
              <dd>{databaseHealth?.migrationCount ?? 0}</dd>
            </div>
          </dl>
        </article>

        <article className="panel wide">
          <h2>Recent Codex Requests</h2>
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
            <p className="empty">
              No Codex requests imported yet. Run <code>Sync Codex Sessions</code> to ingest local
              rollout history.
            </p>
          )}
        </article>

        <article className="panel wide">
          <h2>Recent Claude Code Requests</h2>
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
            <p className="empty">
              No Claude Code requests imported yet. Run <code>Sync Claude Code Sessions</code> to
              ingest local project history.
            </p>
          )}
        </article>

        <article className="panel wide">
          <h2>Schema Summary</h2>
          <div className="table-grid">
            {databaseSummary?.tables.map((table) => (
              <div key={table.tableName} className="table-card">
                <strong>{table.tableName}</strong>
                <span>{formatNumber(table.rowCount)} rows</span>
              </div>
            )) ?? <p className="empty">Waiting for schema summary...</p>}
          </div>
        </article>

        <article className="panel wide">
          <h2>Phase 3 Coverage</h2>
          <ul className="checklist">
            <li>Scans local `~/.codex/sessions/**/*.jsonl` rollout files without proxying traffic.</li>
            <li>Normalizes session metadata, tokens, TTFT, duration, model, and stream heuristic into SQLite.</li>
            <li>Rebuilds `daily_usage` for provider `codex` and exposes today&apos;s totals to the UI.</li>
            <li>Lists recent request records with input/output tokens, TTFT, duration, and stream type.</li>
          </ul>
        </article>

        <article className="panel wide">
          <h2>Phase 4 Coverage</h2>
          <ul className="checklist">
            <li>Scans local `~/.claude/projects/**/*.jsonl` project files without proxying traffic.</li>
            <li>Parses assistant messages for model, token usage, cache tokens, and content summary.</li>
            <li>Reads `~/.claude/sessions/*.json` for session metadata overrides (cwd, entrypoint, startedAt).</li>
            <li>Normalizes session and request records into SQLite with `claude_code` provider.</li>
            <li>Rebuilds `daily_usage` for provider `claude_code` and exposes today&apos;s totals to the UI.</li>
            <li>Lists recent Claude Code request records with input/output/cache tokens and model.</li>
            <li>TTFT and duration_ms are null for passive ingest (available via Managed Launch in future).</li>
          </ul>
        </article>
      </section>
    </main>
  );
}

export default App;
