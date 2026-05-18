use rusqlite::{params, Connection};

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "phase2_initial_schema",
    sql: "
        CREATE TABLE IF NOT EXISTS app_metadata (
          key TEXT PRIMARY KEY NOT NULL,
          value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS schema_migrations (
          version INTEGER PRIMARY KEY NOT NULL,
          name TEXT NOT NULL,
          applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS provider_profiles (
          id TEXT PRIMARY KEY NOT NULL,
          provider_key TEXT NOT NULL UNIQUE,
          display_name TEXT NOT NULL,
          base_url TEXT,
          api_format TEXT NOT NULL,
          api_key_env TEXT,
          enabled INTEGER NOT NULL DEFAULT 1,
          extra_json TEXT,
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS sessions (
          id TEXT PRIMARY KEY NOT NULL,
          provider TEXT NOT NULL,
          source_mode TEXT NOT NULL,
          session_id TEXT NOT NULL,
          cwd TEXT,
          model TEXT,
          entrypoint TEXT,
          started_at TEXT,
          finished_at TEXT,
          metadata_json TEXT,
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          UNIQUE(provider, session_id)
        );

        CREATE TABLE IF NOT EXISTS request_records (
          id TEXT PRIMARY KEY NOT NULL,
          provider TEXT NOT NULL,
          source_mode TEXT NOT NULL,
          session_id TEXT,
          request_id TEXT,
          model TEXT,
          is_stream INTEGER NOT NULL DEFAULT 0,
          input_tokens INTEGER NOT NULL DEFAULT 0,
          output_tokens INTEGER NOT NULL DEFAULT 0,
          cached_input_tokens INTEGER NOT NULL DEFAULT 0,
          reasoning_tokens INTEGER NOT NULL DEFAULT 0,
          ttft_ms INTEGER,
          duration_ms INTEGER,
          status TEXT NOT NULL DEFAULT 'completed',
          started_at TEXT NOT NULL,
          finished_at TEXT,
          request_summary_json TEXT,
          response_summary_json TEXT,
          error_text TEXT,
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS daily_usage (
          date TEXT NOT NULL,
          provider TEXT NOT NULL,
          input_tokens INTEGER NOT NULL DEFAULT 0,
          output_tokens INTEGER NOT NULL DEFAULT 0,
          total_tokens INTEGER NOT NULL DEFAULT 0,
          request_count INTEGER NOT NULL DEFAULT 0,
          stream_count INTEGER NOT NULL DEFAULT 0,
          non_stream_count INTEGER NOT NULL DEFAULT 0,
          avg_ttft_ms REAL,
          avg_duration_ms REAL,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          PRIMARY KEY(date, provider)
        );

        CREATE INDEX IF NOT EXISTS idx_request_records_provider_started_at
          ON request_records(provider, started_at DESC);
        CREATE INDEX IF NOT EXISTS idx_request_records_request_id
          ON request_records(request_id);
        CREATE INDEX IF NOT EXISTS idx_request_records_session_id
          ON request_records(session_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_provider_session_id
          ON sessions(provider, session_id);
        CREATE INDEX IF NOT EXISTS idx_daily_usage_provider_date
          ON daily_usage(provider, date DESC);
    ",
}];

pub fn apply_migrations(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS app_metadata (
              key TEXT PRIMARY KEY NOT NULL,
              value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS schema_migrations (
              version INTEGER PRIMARY KEY NOT NULL,
              name TEXT NOT NULL,
              applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO app_metadata(key, value)
              VALUES ('initialized_at', CURRENT_TIMESTAMP)
              ON CONFLICT(key) DO NOTHING;
            ",
        )
        .map_err(|error| error.to_string())?;

    let mut statement = connection
        .prepare("SELECT version FROM schema_migrations")
        .map_err(|error| error.to_string())?;
    let applied_versions = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    for migration in MIGRATIONS {
        if applied_versions.contains(&migration.version) {
            continue;
        }

        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name) VALUES (?1, ?2)",
                params![migration.version, migration.name],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "
                INSERT INTO app_metadata(key, value)
                VALUES ('schema_version', ?1)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                ",
                [migration.version.to_string()],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "
                INSERT INTO app_metadata(key, value)
                VALUES ('last_migrated_at', CURRENT_TIMESTAMP)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                ",
                [],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
    }

    Ok(())
}
