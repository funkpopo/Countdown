use std::fs;
use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension};
use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::models::DatabaseHealth;

const DATABASE_FILE: &str = "countdown.db";
const SCHEMA_VERSION: &str = "phase1";

pub fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve(DATABASE_FILE, BaseDirectory::AppData)
        .map_err(|error| error.to_string())
}

pub fn initialize(app: &AppHandle) -> Result<(), String> {
    let database_path = database_path(app)?;

    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let connection = Connection::open(&database_path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS app_metadata (
              key TEXT PRIMARY KEY NOT NULL,
              value TEXT NOT NULL
            );
            INSERT INTO app_metadata(key, value)
              VALUES ('schema_version', 'phase1')
              ON CONFLICT(key) DO UPDATE SET value = excluded.value;
            INSERT INTO app_metadata(key, value)
              VALUES ('initialized_at', CURRENT_TIMESTAMP)
              ON CONFLICT(key) DO NOTHING;
            ",
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

pub fn healthcheck(app: &AppHandle) -> Result<DatabaseHealth, String> {
    let database_path = database_path(app)?;
    let exists = database_path.exists();
    let writable = database_path
        .parent()
        .map(|parent| parent.exists())
        .unwrap_or(false);

    if !exists {
        return Ok(DatabaseHealth {
            database_path: database_path.display().to_string(),
            exists,
            writable,
            schema_version: None,
            initialized_at: None,
        });
    }

    let connection = Connection::open(&database_path).map_err(|error| error.to_string())?;
    let schema_version = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .or_else(|| Some(SCHEMA_VERSION.to_string()));

    let initialized_at = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'initialized_at'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    Ok(DatabaseHealth {
        database_path: database_path.display().to_string(),
        exists,
        writable,
        schema_version,
        initialized_at,
    })
}
