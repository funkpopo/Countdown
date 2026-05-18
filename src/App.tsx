import { useEffect, useState, useTransition } from "react";
import {
  databaseHealthcheck,
  getBootstrapInfo,
  getDatabaseSummary,
  initializeLocalDatabase,
  type BootstrapInfo,
  type DatabaseHealth,
  type DatabaseSummary,
} from "./desktop";
import "./App.css";

function App() {
  const [bootstrapInfo, setBootstrapInfo] = useState<BootstrapInfo | null>(null);
  const [databaseHealth, setDatabaseHealth] = useState<DatabaseHealth | null>(null);
  const [databaseSummary, setDatabaseSummary] = useState<DatabaseSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  const refresh = () => {
    startTransition(async () => {
      try {
        setError(null);
        const [bootstrap, health, summary] = await Promise.all([
          getBootstrapInfo(),
          databaseHealthcheck(),
          getDatabaseSummary(),
        ]);
        setBootstrapInfo(bootstrap);
        setDatabaseHealth(health);
        setDatabaseSummary(summary);
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
        const health = await initializeLocalDatabase();
        setDatabaseHealth(health);
        const [bootstrap, summary] = await Promise.all([
          getBootstrapInfo(),
          getDatabaseSummary(),
        ]);
        setBootstrapInfo(bootstrap);
        setDatabaseSummary(summary);
      } catch (initError) {
        setError(
          initError instanceof Error ? initError.message : "Failed to initialize database.",
        );
      }
    });
  };

  return (
    <main className="shell">
      <section className="hero">
        <div>
          <p className="eyebrow">Phase 1 / Engineering Scaffold</p>
          <h1>Countdown Desktop</h1>
          <p className="lede">
            Desktop scaffold for local Claude Code and Codex usage analytics, powered by
            Tauri v2, Rust, Bun, Vite, React, and SQLite.
          </p>
        </div>

        <div className="actions">
          <button type="button" onClick={refresh} disabled={isPending}>
            Refresh State
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

      <section className="grid">
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
              <dd>{bootstrapInfo?.appDataDir ?? "Loading..."}</dd>
            </div>
          </dl>
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
              <dt>IPC</dt>
              <dd>{bootstrapInfo ? "Connected" : "Waiting"}</dd>
            </div>
          </dl>
        </article>

        <article className="panel wide">
          <h2>SQLite Bootstrap</h2>
          <dl className="facts">
            <div>
              <dt>Database Path</dt>
              <dd>{databaseHealth?.databasePath ?? "Not resolved yet"}</dd>
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
              <dt>Schema Version</dt>
              <dd>{databaseHealth?.schemaVersion ?? "Uninitialized"}</dd>
            </div>
            <div>
              <dt>Initialized At</dt>
              <dd>{databaseHealth?.initializedAt ?? "Uninitialized"}</dd>
            </div>
            <div>
              <dt>Migrations</dt>
              <dd>{databaseHealth?.migrationCount ?? 0}</dd>
            </div>
          </dl>
        </article>

        <article className="panel wide">
          <h2>Schema Summary</h2>
          <dl className="facts">
            <div>
              <dt>Schema Version</dt>
              <dd>{databaseSummary?.schemaVersion ?? "Not applied"}</dd>
            </div>
            <div>
              <dt>Applied Migrations</dt>
              <dd>{databaseSummary?.appliedMigrations.length ?? 0}</dd>
            </div>
            <div>
              <dt>Provider Profiles</dt>
              <dd>{databaseSummary?.providerProfiles.length ?? 0}</dd>
            </div>
            <div>
              <dt>Refresh</dt>
              <dd>{isPending ? "Running" : "Idle"}</dd>
            </div>
          </dl>
        </article>

        <article className="panel wide">
          <h2>Core Tables</h2>
          <div className="table-grid">
            {databaseSummary?.tables.map((table) => (
              <div key={table.tableName} className="table-card">
                <strong>{table.tableName}</strong>
                <span>{table.rowCount} rows</span>
              </div>
            )) ?? <p className="empty">Waiting for schema summary...</p>}
          </div>
        </article>

        <article className="panel wide">
          <h2>Phase 2 Coverage</h2>
          <ul className="checklist">
            <li>`request_records` table with token, latency, model, status, and JSON summary fields</li>
            <li>`daily_usage` aggregate table keyed by `date + provider`</li>
            <li>`sessions` table for provider/session-level metadata</li>
            <li>`provider_profiles` table for future local compat routing and upstream credentials</li>
            <li>Migration registry in `schema_migrations` plus repository queries for summary/profile access</li>
          </ul>
        </article>
      </section>
    </main>
  );
}

export default App;
