import { useEffect, useState, useTransition } from "react";
import {
  databaseHealthcheck,
  getBootstrapInfo,
  initializeLocalDatabase,
  type BootstrapInfo,
  type DatabaseHealth,
} from "./desktop";
import "./App.css";

function App() {
  const [bootstrapInfo, setBootstrapInfo] = useState<BootstrapInfo | null>(null);
  const [databaseHealth, setDatabaseHealth] = useState<DatabaseHealth | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  const refresh = () => {
    startTransition(async () => {
      try {
        setError(null);
        const [bootstrap, health] = await Promise.all([
          getBootstrapInfo(),
          databaseHealthcheck(),
        ]);
        setBootstrapInfo(bootstrap);
        setDatabaseHealth(health);
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
        const bootstrap = await getBootstrapInfo();
        setBootstrapInfo(bootstrap);
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
              <dt>IPC</dt>
              <dd>{bootstrapInfo ? "Connected" : "Waiting"}</dd>
            </div>
            <div>
              <dt>Refresh</dt>
              <dd>{isPending ? "Running" : "Idle"}</dd>
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
          </dl>
        </article>

        <article className="panel wide">
          <h2>Scaffold Coverage</h2>
          <ul className="checklist">
            <li>Tauri v2 app shell with Bun / Vite / React / TypeScript</li>
            <li>Rust command IPC wired through `invoke`</li>
            <li>SQLite initialization and health check entry points</li>
            <li>Target module folders for collectors, analytics, tray, compat API, and models</li>
            <li>Ready to continue with Phase 2 schema design</li>
          </ul>
        </article>
      </section>
    </main>
  );
}

export default App;
