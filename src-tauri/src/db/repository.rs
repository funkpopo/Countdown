use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{
    AppliedMigration,
    DatabaseSummary,
    ProviderProfileRecord,
    ProviderProfileUpsertInput,
    TableStat,
};

const CORE_TABLES: &[&str] = &[
    "provider_profiles",
    "sessions",
    "request_records",
    "daily_usage",
    "schema_migrations",
];

pub fn get_database_summary(connection: &Connection) -> Result<DatabaseSummary, String> {
    let schema_version = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let initialized_at = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'initialized_at'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let mut migrations_stmt = connection
        .prepare(
            "
            SELECT version, name, applied_at
            FROM schema_migrations
            ORDER BY version ASC
            ",
        )
        .map_err(|error| error.to_string())?;

    let applied_migrations = migrations_stmt
        .query_map([], |row| {
            Ok(AppliedMigration {
                version: row.get(0)?,
                name: row.get(1)?,
                applied_at: row.get(2)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    let mut tables = Vec::with_capacity(CORE_TABLES.len());
    for table_name in CORE_TABLES {
        tables.push(TableStat {
            table_name: (*table_name).to_string(),
            row_count: query_table_count(connection, table_name)?,
        });
    }

    let provider_profiles = list_provider_profiles(connection)?;

    Ok(DatabaseSummary {
        schema_version,
        initialized_at,
        applied_migrations,
        tables,
        provider_profiles,
    })
}

pub fn list_provider_profiles(connection: &Connection) -> Result<Vec<ProviderProfileRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              provider_key,
              display_name,
              base_url,
              api_format,
              api_key_env,
              enabled,
              extra_json,
              created_at,
              updated_at
            FROM provider_profiles
            ORDER BY display_name ASC
            ",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([], |row| {
            Ok(ProviderProfileRecord {
                id: row.get(0)?,
                provider_key: row.get(1)?,
                display_name: row.get(2)?,
                base_url: row.get(3)?,
                api_format: row.get(4)?,
                api_key_env: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
                extra_json: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn upsert_provider_profile(
    connection: &Connection,
    input: &ProviderProfileUpsertInput,
) -> Result<ProviderProfileRecord, String> {
    connection
        .execute(
            "
            INSERT INTO provider_profiles (
              id,
              provider_key,
              display_name,
              base_url,
              api_format,
              api_key_env,
              enabled,
              extra_json,
              created_at,
              updated_at
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            )
            ON CONFLICT(id) DO UPDATE SET
              provider_key = excluded.provider_key,
              display_name = excluded.display_name,
              base_url = excluded.base_url,
              api_format = excluded.api_format,
              api_key_env = excluded.api_key_env,
              enabled = excluded.enabled,
              extra_json = excluded.extra_json,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                input.id,
                input.provider_key,
                input.display_name,
                input.base_url,
                input.api_format,
                input.api_key_env,
                i64::from(input.enabled),
                input.extra_json,
            ],
        )
        .map_err(|error| error.to_string())?;

    connection
        .query_row(
            "
            SELECT
              id,
              provider_key,
              display_name,
              base_url,
              api_format,
              api_key_env,
              enabled,
              extra_json,
              created_at,
              updated_at
            FROM provider_profiles
            WHERE id = ?1
            ",
            [input.id.as_str()],
            |row| {
                Ok(ProviderProfileRecord {
                    id: row.get(0)?,
                    provider_key: row.get(1)?,
                    display_name: row.get(2)?,
                    base_url: row.get(3)?,
                    api_format: row.get(4)?,
                    api_key_env: row.get(5)?,
                    enabled: row.get::<_, i64>(6)? != 0,
                    extra_json: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
        .map_err(|error| error.to_string())
}

fn query_table_count(connection: &Connection, table_name: &str) -> Result<i64, String> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    connection
        .query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map_err(|error| error.to_string())
}
