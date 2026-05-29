use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::collectors::claude_code::default_claude_data_dir;
use crate::collectors::codex::default_codex_sessions_dir;
use crate::db;

const ACTIVE_POLL_INTERVAL: Duration = Duration::from_secs(3);
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(15);
const QUIET_TICKS_BEFORE_IDLE: u32 = 20;
const SYNC_COMPLETED_EVENT: &str = "usage-sync-completed";
const SYNC_FAILED_EVENT: &str = "usage-sync-failed";

static SYNC_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_fingerprint = None;
        let mut quiet_ticks = 0;
        let mut next_interval = ACTIVE_POLL_INTERVAL;

        // Brief delay so the Tauri window can render before heavy sync work
        tokio::time::sleep(Duration::from_millis(500)).await;

        let first_tick = run_once(app.clone(), &mut last_fingerprint, true).await;
        apply_tick_result(first_tick, &mut quiet_ticks, &mut next_interval);

        loop {
            tokio::time::sleep(next_interval).await;
            let tick = run_once(app.clone(), &mut last_fingerprint, false).await;
            apply_tick_result(tick, &mut quiet_ticks, &mut next_interval);
        }
    });
}

pub fn sync_now(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut fingerprint = None;
        let _ = run_once(app, &mut fingerprint, true).await;
    });
}

async fn run_once(
    app: AppHandle,
    last_fingerprint: &mut Option<SourceFingerprint>,
    force: bool,
) -> PollTickResult {
    if SYNC_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return PollTickResult::Busy;
    }

    let app_for_sync = app.clone();
    let previous_fingerprint = last_fingerprint.clone();
    let started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let current_fingerprint = source_fingerprint()?;

        if !force && previous_fingerprint.as_ref() == Some(&current_fingerprint) {
            return Ok::<_, String>(SyncTickResult::Unchanged(current_fingerprint));
        }

        db::initialize(&app_for_sync)?;
        let (codex, claude) = db::sync_all_sessions(&app_for_sync)?;

        Ok::<_, String>(SyncTickResult::Synced {
            fingerprint: current_fingerprint,
            payload: json!({
                "codexImportedRequests": codex.imported_requests,
                "claudeImportedRequests": claude.imported_requests,
                "codexRequestCount": codex.request_count,
                "claudeRequestCount": claude.request_count,
                "syncedAt": chrono::Utc::now().to_rfc3339(),
            }),
        })
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(|result| result);
    let elapsed_ms = started.elapsed().as_millis() as u64;

    SYNC_RUNNING.store(false, Ordering::SeqCst);

    match result {
        Ok(SyncTickResult::Synced {
            fingerprint,
            payload,
        }) => {
            *last_fingerprint = Some(fingerprint);
            let _ = app.emit(SYNC_COMPLETED_EVENT, payload);
            PollTickResult::Synced { elapsed_ms }
        }
        Ok(SyncTickResult::Unchanged(fingerprint)) => {
            *last_fingerprint = Some(fingerprint);
            PollTickResult::Unchanged { elapsed_ms }
        }
        Err(error) => {
            let _ = app.emit(
                SYNC_FAILED_EVENT,
                json!({
                    "error": error,
                    "syncedAt": chrono::Utc::now().to_rfc3339(),
                }),
            );
            PollTickResult::Failed
        }
    }
}

fn apply_tick_result(tick: PollTickResult, quiet_ticks: &mut u32, next_interval: &mut Duration) {
    match tick {
        PollTickResult::Synced { elapsed_ms } => {
            *quiet_ticks = 0;
            *next_interval = if elapsed_ms > 2_000 {
                IDLE_POLL_INTERVAL
            } else {
                ACTIVE_POLL_INTERVAL
            };
        }
        PollTickResult::Unchanged { elapsed_ms } => {
            *quiet_ticks = quiet_ticks.saturating_add(1);
            *next_interval = if *quiet_ticks >= QUIET_TICKS_BEFORE_IDLE || elapsed_ms > 1_000 {
                IDLE_POLL_INTERVAL
            } else {
                ACTIVE_POLL_INTERVAL
            };
        }
        PollTickResult::Busy | PollTickResult::Failed => {
            *next_interval = IDLE_POLL_INTERVAL;
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct SourceFingerprint {
    file_count: u64,
    total_bytes: u64,
    latest_modified_ms: u128,
}

enum SyncTickResult {
    Synced {
        fingerprint: SourceFingerprint,
        payload: serde_json::Value,
    },
    Unchanged(SourceFingerprint),
}

enum PollTickResult {
    Synced { elapsed_ms: u64 },
    Unchanged { elapsed_ms: u64 },
    Busy,
    Failed,
}

fn source_fingerprint() -> Result<SourceFingerprint, String> {
    let mut fingerprint = SourceFingerprint::default();

    if let Ok(codex_dir) = default_codex_sessions_dir() {
        collect_fingerprint(&codex_dir, &mut fingerprint)?;
    }

    if let Ok(claude_dir) = default_claude_data_dir() {
        collect_fingerprint(&claude_dir.join("projects"), &mut fingerprint)?;
        collect_fingerprint(&claude_dir.join("sessions"), &mut fingerprint)?;
    }

    Ok(fingerprint)
}

fn collect_fingerprint(path: &Path, fingerprint: &mut SourceFingerprint) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|error| error.to_string())?;

        if metadata.is_dir() {
            collect_fingerprint(&path, fingerprint)?;
            continue;
        }

        if !metadata.is_file() || !is_supported_source_file(&path) {
            continue;
        }

        fingerprint.file_count += 1;
        fingerprint.total_bytes = fingerprint.total_bytes.saturating_add(metadata.len());

        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                fingerprint.latest_modified_ms =
                    fingerprint.latest_modified_ms.max(duration.as_millis());
            }
        }
    }

    Ok(())
}

fn is_supported_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("jsonl") || extension.eq_ignore_ascii_case("json")
        })
}
