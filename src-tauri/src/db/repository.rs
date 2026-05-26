use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{
    AppliedMigration, CombinedTodayUsage, CombinedUsage, DailyUsageRecord, DatabaseSummary,
    PaginatedRequestRecords, ProviderProfileRecord, ProviderProfileUpsertInput, RequestFilterInput,
    RequestRecordDetail, RequestRecordListItem, RequestRecordUpsertRecord, SessionUpsertRecord,
    TableStat, UsageHistogram, UsageHistogramBucket,
};

const CORE_TABLES: &[&str] = &[
    "provider_profiles",
    "sessions",
    "request_records",
    "daily_usage",
    "schema_migrations",
];

const REQUEST_RECORDS_RECENT_ORDER: &str = "
    CASE WHEN COALESCE(rr.finished_at, rr.started_at) IS NULL THEN 1 ELSE 0 END ASC,
    unixepoch(COALESCE(rr.finished_at, rr.started_at)) DESC,
    COALESCE(rr.finished_at, rr.started_at) DESC,
    rr.updated_at DESC
";

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

pub fn list_provider_profiles(
    connection: &Connection,
) -> Result<Vec<ProviderProfileRecord>, String> {
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
    let target_id = resolve_provider_profile_target_id(connection, input)?;

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
                target_id,
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

    get_provider_profile_by_id(connection, &target_id)
}

pub fn delete_provider_profile(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM provider_profiles WHERE id = ?1", [id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn upsert_provider_profiles(
    connection: &mut Connection,
    inputs: &[ProviderProfileUpsertInput],
) -> Result<Vec<ProviderProfileRecord>, String> {
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    let mut records = Vec::with_capacity(inputs.len());

    for input in inputs {
        records.push(upsert_provider_profile(&transaction, input)?);
    }

    transaction.commit().map_err(|error| error.to_string())?;
    Ok(records)
}

fn resolve_provider_profile_target_id(
    connection: &Connection,
    input: &ProviderProfileUpsertInput,
) -> Result<String, String> {
    let existing_by_id = connection
        .query_row(
            "SELECT id FROM provider_profiles WHERE id = ?1",
            [input.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    if let Some(existing_id) = existing_by_id {
        return Ok(existing_id);
    }

    let existing_by_key = connection
        .query_row(
            "SELECT id FROM provider_profiles WHERE provider_key = ?1",
            [input.provider_key.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    Ok(existing_by_key.unwrap_or_else(|| input.id.clone()))
}

fn get_provider_profile_by_id(
    connection: &Connection,
    id: &str,
) -> Result<ProviderProfileRecord, String> {
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
            [id],
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

pub fn upsert_session_record(
    connection: &Connection,
    record: &SessionUpsertRecord,
) -> Result<(), String> {
    connection
        .execute(
            "
            INSERT INTO sessions (
              id,
              provider,
              source_mode,
              session_id,
              cwd,
              model,
              entrypoint,
              started_at,
              finished_at,
              metadata_json,
              created_at,
              updated_at
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            )
            ON CONFLICT(id) DO UPDATE SET
              provider = excluded.provider,
              source_mode = excluded.source_mode,
              session_id = excluded.session_id,
              cwd = excluded.cwd,
              model = excluded.model,
              entrypoint = excluded.entrypoint,
              started_at = excluded.started_at,
              finished_at = excluded.finished_at,
              metadata_json = excluded.metadata_json,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                record.id,
                record.provider,
                record.source_mode,
                record.session_id,
                record.cwd,
                record.model,
                record.entrypoint,
                record.started_at,
                record.finished_at,
                record.metadata_json,
            ],
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

pub fn upsert_request_record(
    connection: &Connection,
    record: &RequestRecordUpsertRecord,
) -> Result<(), String> {
    connection
        .execute(
            "
            INSERT INTO request_records (
              id,
              provider,
              source_mode,
              session_id,
              request_id,
              model,
              is_stream,
              input_tokens,
              output_tokens,
              cached_input_tokens,
              reasoning_tokens,
              ttft_ms,
              duration_ms,
              status,
              started_at,
              finished_at,
              request_summary_json,
              response_summary_json,
              error_text,
              created_at,
              updated_at
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
              CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            )
            ON CONFLICT(id) DO UPDATE SET
              provider = excluded.provider,
              source_mode = excluded.source_mode,
              session_id = excluded.session_id,
              request_id = excluded.request_id,
              model = excluded.model,
              is_stream = excluded.is_stream,
              input_tokens = excluded.input_tokens,
              output_tokens = excluded.output_tokens,
              cached_input_tokens = excluded.cached_input_tokens,
              reasoning_tokens = excluded.reasoning_tokens,
              ttft_ms = excluded.ttft_ms,
              duration_ms = excluded.duration_ms,
              status = excluded.status,
              started_at = excluded.started_at,
              finished_at = excluded.finished_at,
              request_summary_json = excluded.request_summary_json,
              response_summary_json = excluded.response_summary_json,
              error_text = excluded.error_text,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                record.id,
                record.provider,
                record.source_mode,
                record.session_id,
                record.request_id,
                record.model,
                i64::from(record.is_stream),
                record.input_tokens,
                record.output_tokens,
                record.cached_input_tokens,
                record.reasoning_tokens,
                record.ttft_ms,
                record.duration_ms,
                record.status,
                record.started_at,
                record.finished_at,
                record.request_summary_json,
                record.response_summary_json,
                record.error_text,
            ],
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

pub fn rebuild_daily_usage_for_provider(
    connection: &Connection,
    provider: &str,
) -> Result<(), String> {
    connection
        .execute("DELETE FROM daily_usage WHERE provider = ?1", [provider])
        .map_err(|error| error.to_string())?;

    connection
        .execute(
            "
            INSERT INTO daily_usage (
              date,
              provider,
              input_tokens,
              cached_input_tokens,
              output_tokens,
              total_tokens,
              request_count,
              stream_count,
              non_stream_count,
              avg_ttft_ms,
              avg_duration_ms,
              updated_at
            )
            SELECT
              DATE(COALESCE(finished_at, started_at), 'localtime') AS usage_date,
              provider,
              COALESCE(SUM(input_tokens), 0),
              COALESCE(SUM(cached_input_tokens), 0),
              COALESCE(SUM(output_tokens), 0),
              COALESCE(SUM(input_tokens + output_tokens), 0),
              COUNT(*),
              COALESCE(SUM(CASE WHEN is_stream = 1 THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN is_stream = 0 THEN 1 ELSE 0 END), 0),
              AVG(ttft_ms),
              AVG(duration_ms),
              CURRENT_TIMESTAMP
            FROM request_records
            WHERE provider = ?1
            GROUP BY usage_date, provider
            ",
            [provider],
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

pub fn get_provider_today_usage(
    connection: &Connection,
    provider: &str,
) -> Result<Option<DailyUsageRecord>, String> {
    connection
        .query_row(
            "
            SELECT
              date,
              provider,
              input_tokens,
              cached_input_tokens,
              output_tokens,
              total_tokens,
              request_count,
              stream_count,
              non_stream_count,
              avg_ttft_ms,
              avg_duration_ms,
              updated_at
            FROM daily_usage
            WHERE provider = ?1
              AND date = DATE('now', 'localtime')
            ",
            [provider],
            map_daily_usage_row,
        )
        .optional()
        .map_err(|error| error.to_string())
}

pub fn get_combined_today_usage(connection: &Connection) -> Result<CombinedTodayUsage, String> {
    let claude_usage = get_provider_today_usage(connection, "claude_code")?;
    let codex_usage = get_provider_today_usage(connection, "codex")?;

    let claude_input = claude_usage.as_ref().map(|u| u.input_tokens).unwrap_or(0);
    let claude_cached = claude_usage
        .as_ref()
        .map(|u| u.cached_input_tokens)
        .unwrap_or(0);
    let claude_output = claude_usage.as_ref().map(|u| u.output_tokens).unwrap_or(0);
    let claude_total = claude_usage.as_ref().map(|u| u.total_tokens).unwrap_or(0);
    let claude_requests = claude_usage.as_ref().map(|u| u.request_count).unwrap_or(0);

    let codex_input = codex_usage.as_ref().map(|u| u.input_tokens).unwrap_or(0);
    let codex_cached = codex_usage
        .as_ref()
        .map(|u| u.cached_input_tokens)
        .unwrap_or(0);
    let codex_output = codex_usage.as_ref().map(|u| u.output_tokens).unwrap_or(0);
    let codex_total = codex_usage.as_ref().map(|u| u.total_tokens).unwrap_or(0);
    let codex_requests = codex_usage.as_ref().map(|u| u.request_count).unwrap_or(0);

    let now = chrono::Utc::now();
    let date = now.format("%Y-%m-%d").to_string();
    let last_refresh_at = now.to_rfc3339();

    Ok(CombinedTodayUsage {
        date,
        claude_input_tokens: claude_input,
        claude_cached_input_tokens: claude_cached,
        claude_output_tokens: claude_output,
        claude_total_tokens: claude_total,
        claude_request_count: claude_requests,
        codex_input_tokens: codex_input,
        codex_cached_input_tokens: codex_cached,
        codex_output_tokens: codex_output,
        codex_total_tokens: codex_total,
        codex_request_count: codex_requests,
        combined_input_tokens: claude_input + codex_input,
        combined_cached_input_tokens: claude_cached + codex_cached,
        combined_output_tokens: claude_output + codex_output,
        combined_total_tokens: claude_total + codex_total,
        combined_request_count: claude_requests + codex_requests,
        last_refresh_at,
    })
}

fn get_provider_agg_for_range(
    connection: &Connection,
    provider: &str,
    start_date: &str,
    end_date: &str,
) -> Result<(i64, i64, i64, i64, i64), String> {
    connection
        .query_row(
            "SELECT
               COALESCE(SUM(input_tokens), 0),
               COALESCE(SUM(cached_input_tokens), 0),
               COALESCE(SUM(output_tokens), 0),
               COALESCE(SUM(total_tokens), 0),
               COALESCE(SUM(request_count), 0)
             FROM daily_usage
             WHERE provider = ?1 AND date >= ?2 AND date <= ?3",
            params![provider, start_date, end_date],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .map_err(|error| error.to_string())
}

pub fn get_combined_usage_for_range(
    connection: &Connection,
    start_date: &str,
    end_date: &str,
) -> Result<CombinedUsage, String> {
    let (ci, cc, co, ct, cr) =
        get_provider_agg_for_range(connection, "claude_code", start_date, end_date)?;
    let (xi, xc, xo, xt, xr) =
        get_provider_agg_for_range(connection, "codex", start_date, end_date)?;

    Ok(CombinedUsage {
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        claude_input_tokens: ci,
        claude_cached_input_tokens: cc,
        claude_output_tokens: co,
        claude_total_tokens: ct,
        claude_request_count: cr,
        codex_input_tokens: xi,
        codex_cached_input_tokens: xc,
        codex_output_tokens: xo,
        codex_total_tokens: xt,
        codex_request_count: xr,
        combined_input_tokens: ci + xi,
        combined_cached_input_tokens: cc + xc,
        combined_output_tokens: co + xo,
        combined_total_tokens: ct + xt,
        combined_request_count: cr + xr,
        last_refresh_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn get_provider_total_usage(
    connection: &Connection,
    provider: &str,
) -> Result<(i64, i64, i64, i64), String> {
    connection
        .query_row(
            "
            SELECT
              COALESCE(SUM(input_tokens), 0),
              COALESCE(SUM(cached_input_tokens), 0),
              COALESCE(SUM(output_tokens), 0),
              COUNT(*)
            FROM request_records
            WHERE provider = ?1
            ",
            [provider],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(|error| error.to_string())
}

pub fn get_combined_usage_total(connection: &Connection) -> Result<CombinedUsage, String> {
    let (ci, cc, co, cr) = get_provider_total_usage(connection, "claude_code")?;
    let (xi, xc, xo, xr) = get_provider_total_usage(connection, "codex")?;
    let (start_date, end_date) = connection
        .query_row(
            "
            SELECT
              COALESCE(MIN(DATE(COALESCE(finished_at, started_at), 'localtime')), ''),
              COALESCE(MAX(DATE(COALESCE(finished_at, started_at), 'localtime')), '')
            FROM request_records
            WHERE provider IN ('claude_code', 'codex')
            ",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|error| error.to_string())?;

    Ok(CombinedUsage {
        start_date,
        end_date,
        claude_input_tokens: ci,
        claude_cached_input_tokens: cc,
        claude_output_tokens: co,
        claude_total_tokens: ci + co,
        claude_request_count: cr,
        codex_input_tokens: xi,
        codex_cached_input_tokens: xc,
        codex_output_tokens: xo,
        codex_total_tokens: xi + xo,
        codex_request_count: xr,
        combined_input_tokens: ci + xi,
        combined_cached_input_tokens: cc + xc,
        combined_output_tokens: co + xo,
        combined_total_tokens: ci + co + xi + xo,
        combined_request_count: cr + xr,
        last_refresh_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn get_usage_histogram(
    connection: &Connection,
    period: &str,
    granularity: &str,
) -> Result<UsageHistogram, String> {
    let (bucket_seed, bucket_next, bucket_limit, bucket_expr, where_sql) = match (period, granularity)
    {
        ("today", "hour") => (
            "strftime('%Y-%m-%d %H:00', DATE('now', 'localtime'))",
            "strftime('%Y-%m-%d %H:00', datetime(bucket, '+1 hour'))",
            "bucket < strftime('%Y-%m-%d %H:00', datetime(DATE('now', 'localtime'), '+23 hours'))",
            "strftime('%Y-%m-%d %H:00', COALESCE(finished_at, started_at), 'localtime')",
            "DATE(COALESCE(finished_at, started_at), 'localtime') = DATE('now', 'localtime')",
        ),
        ("week", "day") => (
            "DATE('now', 'localtime', 'weekday 1', '-7 days')",
            "DATE(bucket, '+1 day')",
            "bucket < DATE('now', 'localtime')",
            "DATE(COALESCE(finished_at, started_at), 'localtime')",
            "DATE(COALESCE(finished_at, started_at), 'localtime') >= DATE('now', 'localtime', 'weekday 1', '-7 days') AND DATE(COALESCE(finished_at, started_at), 'localtime') <= DATE('now', 'localtime')",
        ),
        ("month", "day") => (
            "DATE('now', 'localtime', 'start of month')",
            "DATE(bucket, '+1 day')",
            "bucket < DATE('now', 'localtime')",
            "DATE(COALESCE(finished_at, started_at), 'localtime')",
            "DATE(COALESCE(finished_at, started_at), 'localtime') >= DATE('now', 'localtime', 'start of month') AND DATE(COALESCE(finished_at, started_at), 'localtime') <= DATE('now', 'localtime')",
        ),
        _ => return Err("Unsupported histogram period or granularity".to_string()),
    };

    let sql = format!(
        "
        WITH RECURSIVE buckets(bucket) AS (
          SELECT {bucket_seed}
          UNION ALL
          SELECT {bucket_next}
          FROM buckets
          WHERE {bucket_limit}
        ),
        usage AS (
          SELECT
            {bucket_expr} AS bucket,
            provider,
            input_tokens,
            cached_input_tokens,
            output_tokens
          FROM request_records
          WHERE provider IN ('claude_code', 'codex') AND {where_sql}
        )
        SELECT
          buckets.bucket,
          COALESCE(SUM(CASE WHEN usage.provider = 'claude_code' THEN usage.input_tokens ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN usage.provider = 'claude_code' THEN usage.cached_input_tokens ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN usage.provider = 'claude_code' THEN usage.output_tokens ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN usage.provider = 'claude_code' THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN usage.provider = 'codex' THEN usage.input_tokens ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN usage.provider = 'codex' THEN usage.cached_input_tokens ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN usage.provider = 'codex' THEN usage.output_tokens ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN usage.provider = 'codex' THEN 1 ELSE 0 END), 0)
        FROM buckets
        LEFT JOIN usage ON usage.bucket = buckets.bucket
        GROUP BY buckets.bucket
        ORDER BY buckets.bucket ASC
        "
    );

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let bucket: String = row.get(0)?;
            let ci: i64 = row.get(1)?;
            let cc: i64 = row.get(2)?;
            let co: i64 = row.get(3)?;
            let cr: i64 = row.get(4)?;
            let xi: i64 = row.get(5)?;
            let xc: i64 = row.get(6)?;
            let xo: i64 = row.get(7)?;
            let xr: i64 = row.get(8)?;
            Ok(UsageHistogramBucket {
                label: bucket.clone(),
                bucket,
                claude_input_tokens: ci,
                claude_cached_input_tokens: cc,
                claude_output_tokens: co,
                claude_total_tokens: ci + co,
                claude_request_count: cr,
                codex_input_tokens: xi,
                codex_cached_input_tokens: xc,
                codex_output_tokens: xo,
                codex_total_tokens: xi + xo,
                codex_request_count: xr,
                combined_input_tokens: ci + xi,
                combined_cached_input_tokens: cc + xc,
                combined_output_tokens: co + xo,
                combined_total_tokens: ci + co + xi + xo,
                combined_request_count: cr + xr,
            })
        })
        .map_err(|error| error.to_string())?;

    Ok(UsageHistogram {
        period: period.to_string(),
        granularity: granularity.to_string(),
        buckets: rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?,
        last_refresh_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn list_recent_request_records(
    connection: &Connection,
    provider: &str,
    limit: i64,
) -> Result<Vec<RequestRecordListItem>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              rr.id,
              rr.provider,
              rr.source_mode,
              rr.session_id,
              rr.request_id,
              rr.model,
              rr.is_stream,
              rr.input_tokens,
              rr.output_tokens,
              rr.cached_input_tokens,
              rr.reasoning_tokens,
              rr.ttft_ms,
              rr.duration_ms,
              rr.status,
              rr.started_at,
              rr.finished_at,
              s.cwd,
              s.entrypoint
            FROM request_records rr
            LEFT JOIN sessions s
              ON rr.provider = s.provider
             AND rr.session_id = s.session_id
            WHERE rr.provider = ?1
            ORDER BY
              CASE WHEN COALESCE(rr.finished_at, rr.started_at) IS NULL THEN 1 ELSE 0 END ASC,
              unixepoch(COALESCE(rr.finished_at, rr.started_at)) DESC,
              COALESCE(rr.finished_at, rr.started_at) DESC,
              rr.updated_at DESC
            LIMIT ?2
            ",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map(params![provider, limit], map_request_record_row)
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn get_provider_counts(connection: &Connection, provider: &str) -> Result<(i64, i64), String> {
    let session_count = connection
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE provider = ?1",
            [provider],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?;

    let request_count = connection
        .query_row(
            "SELECT COUNT(*) FROM request_records WHERE provider = ?1",
            [provider],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?;

    Ok((session_count, request_count))
}

fn map_daily_usage_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DailyUsageRecord> {
    Ok(DailyUsageRecord {
        date: row.get(0)?,
        provider: row.get(1)?,
        input_tokens: row.get(2)?,
        cached_input_tokens: row.get(3)?,
        output_tokens: row.get(4)?,
        total_tokens: row.get(5)?,
        request_count: row.get(6)?,
        stream_count: row.get(7)?,
        non_stream_count: row.get(8)?,
        avg_ttft_ms: row.get(9)?,
        avg_duration_ms: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn map_request_record_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestRecordListItem> {
    Ok(RequestRecordListItem {
        id: row.get(0)?,
        provider: row.get(1)?,
        source_mode: row.get(2)?,
        session_id: row.get(3)?,
        request_id: row.get(4)?,
        model: row.get(5)?,
        is_stream: row.get::<_, i64>(6)? != 0,
        input_tokens: row.get(7)?,
        output_tokens: row.get(8)?,
        cached_input_tokens: row.get(9)?,
        reasoning_tokens: row.get(10)?,
        ttft_ms: row.get(11)?,
        duration_ms: row.get(12)?,
        status: row.get(13)?,
        started_at: row.get(14)?,
        finished_at: row.get(15)?,
        cwd: row.get(16)?,
        entrypoint: row.get(17)?,
    })
}

fn query_table_count(connection: &Connection, table_name: &str) -> Result<i64, String> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    connection
        .query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map_err(|error| error.to_string())
}

pub fn list_filtered_request_records(
    connection: &Connection,
    filter: &RequestFilterInput,
) -> Result<PaginatedRequestRecords, String> {
    let limit = filter.limit.unwrap_or(50);
    let offset = filter.offset.unwrap_or(0);

    let mut where_clauses = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut param_index = 1;

    if let Some(ref provider) = filter.provider {
        where_clauses.push(format!("rr.provider = ?{}", param_index));
        param_values.push(Box::new(provider.clone()));
        param_index += 1;
    }

    if let Some(ref model) = filter.model {
        where_clauses.push(format!("rr.model = ?{}", param_index));
        param_values.push(Box::new(model.clone()));
        param_index += 1;
    }

    if let Some(is_stream) = filter.is_stream {
        where_clauses.push(format!("rr.is_stream = ?{}", param_index));
        param_values.push(Box::new(i64::from(is_stream)));
        param_index += 1;
    }

    if let Some(ref after) = filter.started_after {
        where_clauses.push(format!("rr.started_at >= ?{}", param_index));
        param_values.push(Box::new(after.clone()));
        param_index += 1;
    }

    if let Some(ref before) = filter.started_before {
        where_clauses.push(format!("rr.started_at <= ?{}", param_index));
        param_values.push(Box::new(before.clone()));
        param_index += 1;
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    let summary_sql = format!(
        "
        SELECT
          COUNT(*),
          COALESCE(SUM(rr.input_tokens), 0),
          COALESCE(SUM(rr.cached_input_tokens), 0),
          COALESCE(SUM(rr.output_tokens), 0),
          COALESCE(SUM(rr.reasoning_tokens), 0)
        FROM request_records rr
        {}
        ",
        where_sql
    );

    let (
        total,
        total_input_tokens,
        total_cached_input_tokens,
        total_output_tokens,
        total_reasoning_tokens,
    ): (i64, i64, i64, i64, i64) = connection
        .query_row(
            &summary_sql,
            rusqlite::params_from_iter(param_values.iter().map(|v| v.as_ref())),
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?;

    let data_sql = format!(
        "
        SELECT
          rr.id,
          rr.provider,
          rr.source_mode,
          rr.session_id,
          rr.request_id,
          rr.model,
          rr.is_stream,
          rr.input_tokens,
          rr.output_tokens,
          rr.cached_input_tokens,
          rr.reasoning_tokens,
          rr.ttft_ms,
          rr.duration_ms,
          rr.status,
          rr.started_at,
          rr.finished_at,
          s.cwd,
          s.entrypoint
        FROM request_records rr
        LEFT JOIN sessions s
          ON rr.provider = s.provider
         AND rr.session_id = s.session_id
        {}
        ORDER BY {REQUEST_RECORDS_RECENT_ORDER}
        LIMIT ?{} OFFSET ?{}
        ",
        where_sql,
        param_index,
        param_index + 1
    );

    let mut param_values_with_limit = param_values;
    param_values_with_limit.push(Box::new(limit));
    param_values_with_limit.push(Box::new(offset));

    let mut statement = connection
        .prepare(&data_sql)
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map(
            rusqlite::params_from_iter(param_values_with_limit.iter().map(|v| v.as_ref())),
            map_request_record_row,
        )
        .map_err(|error| error.to_string())?;

    let records = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    Ok(PaginatedRequestRecords {
        records,
        total,
        total_input_tokens,
        total_cached_input_tokens,
        total_output_tokens,
        total_reasoning_tokens,
        limit,
        offset,
    })
}

pub fn get_request_record_detail(
    connection: &Connection,
    id: &str,
) -> Result<RequestRecordDetail, String> {
    connection
        .query_row(
            "
            SELECT
              rr.id,
              rr.provider,
              rr.source_mode,
              rr.session_id,
              rr.request_id,
              rr.model,
              rr.is_stream,
              rr.input_tokens,
              rr.output_tokens,
              rr.cached_input_tokens,
              rr.reasoning_tokens,
              rr.ttft_ms,
              rr.duration_ms,
              rr.status,
              rr.started_at,
              rr.finished_at,
              s.cwd,
              s.entrypoint,
              rr.request_summary_json,
              rr.response_summary_json,
              rr.error_text
            FROM request_records rr
            LEFT JOIN sessions s
              ON rr.provider = s.provider
             AND rr.session_id = s.session_id
            WHERE rr.id = ?1
            ",
            [id],
            map_request_record_detail_row,
        )
        .map_err(|error| error.to_string())
}

fn map_request_record_detail_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestRecordDetail> {
    Ok(RequestRecordDetail {
        id: row.get(0)?,
        provider: row.get(1)?,
        source_mode: row.get(2)?,
        session_id: row.get(3)?,
        request_id: row.get(4)?,
        model: row.get(5)?,
        is_stream: row.get::<_, i64>(6)? != 0,
        input_tokens: row.get(7)?,
        output_tokens: row.get(8)?,
        cached_input_tokens: row.get(9)?,
        reasoning_tokens: row.get(10)?,
        ttft_ms: row.get(11)?,
        duration_ms: row.get(12)?,
        status: row.get(13)?,
        started_at: row.get(14)?,
        finished_at: row.get(15)?,
        cwd: row.get(16)?,
        entrypoint: row.get(17)?,
        request_summary_json: row.get(18)?,
        response_summary_json: row.get(19)?,
        error_text: row.get(20)?,
    })
}
