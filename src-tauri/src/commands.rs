use crate::core::db;
use crate::core::error::{CoreError, Result};
use crate::core::indexer::{IndexJob, IndexResult, Indexer};
use crate::core::queries;
use crate::core::types::*;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub jobs: Mutex<HashMap<u64, Arc<IndexJob>>>,
    pub next_job_id: AtomicU64,
}

fn with_db<T>(state: &AppState, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let guard = state.db.lock().unwrap();
    let conn = guard
        .as_ref()
        .ok_or_else(|| CoreError::other("no database open"))?;
    f(conn)
}

fn with_db_mut<T>(state: &AppState, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
    let mut guard = state.db.lock().unwrap();
    let conn = guard
        .as_mut()
        .ok_or_else(|| CoreError::other("no database open"))?;
    f(conn)
}

fn err(e: CoreError) -> String {
    e.to_string()
}

fn app_db_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| CoreError::other(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("archaeologist.db"))
}

#[tauri::command]
pub fn open_app_database(app: AppHandle) -> std::result::Result<(), String> {
    let path = app_db_path(&app).map_err(err)?;
    let conn = db::open(&path).map_err(err)?;
    let state = app.state::<AppState>();
    *state.db.lock().unwrap() = Some(conn);
    Ok(())
}

#[tauri::command]
pub fn list_repositories(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<RepoSummary>, String> {
    with_db(&state, queries::list_repositories).map_err(err)
}

#[tauri::command]
pub fn open_repository(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> std::result::Result<RepoSummary, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }

    // Ensure DB is open (e.g. dev restart).
    {
        let guard = state.db.lock().unwrap();
        if guard.is_none() {
            drop(guard);
            open_app_database(app)?;
        }
    }

    with_db_mut(&state, |conn| {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(conn, root.clone(), &job, &mut noop);
        let repo_id = indexer.ensure_repository()?;
        queries::summary(conn, &repo_id)
    })
    .map_err(err)
}

#[tauri::command]
pub fn get_summary(
    state: State<'_, AppState>,
    repository_id: String,
) -> std::result::Result<RepoSummary, String> {
    with_db(&state, |c| queries::summary(c, &repository_id)).map_err(err)
}

#[derive(Serialize)]
pub struct StartIndexResponse {
    pub job_id: u64,
}

#[tauri::command]
pub fn start_index(
    app: AppHandle,
    state: State<'_, AppState>,
    repository_id: String,
    root: String,
    mode: String,
) -> std::result::Result<StartIndexResponse, String> {
    // repository_id is currently derived from the root path inside the indexer.
    let _ = &repository_id;
    let job = Arc::new(IndexJob::new());
    let job_id = state.next_job_id.fetch_add(1, Ordering::SeqCst) + 1;
    state.jobs.lock().unwrap().insert(job_id, job.clone());

    let conn_path = app_db_path(&app).map_err(err)?;
    let root_path = PathBuf::from(root);
    let handle = app.clone();

    std::thread::spawn(move || {
        let mut emit = |ev: ProgressEvent| {
            let _ = handle.emit("index-progress", &ev);
        };
        let outcome = (|| -> Result<IndexResult> {
            let mut conn = db::open(&conn_path)?;
            let mut indexer = Indexer::new(&mut conn, root_path, &job, &mut emit);
            indexer.index(&mode)
        })();

        match outcome {
            Ok(res) => {
                let _ = handle.emit("index-completed", &res);
            }
            Err(e) => {
                let cancelled = matches!(e, CoreError::Cancelled);
                let _ = handle.emit(
                    "index-failed",
                    serde_json::json!({ "message": e.to_string(), "cancelled": cancelled }),
                );
            }
        }
        handle
            .state::<AppState>()
            .jobs
            .lock()
            .unwrap()
            .remove(&job_id);
    });

    Ok(StartIndexResponse { job_id })
}

#[tauri::command]
pub fn cancel_index(state: State<'_, AppState>, job_id: u64) -> std::result::Result<(), String> {
    if let Some(job) = state.jobs.lock().unwrap().get(&job_id) {
        job.cancel();
    }
    Ok(())
}

#[tauri::command]
pub fn list_files(
    state: State<'_, AppState>,
    repository_id: String,
    limit: u32,
    offset: u32,
) -> std::result::Result<Vec<FileRecord>, String> {
    with_db(&state, |c| {
        queries::list_files(c, &repository_id, limit, offset)
    })
    .map_err(err)
}

#[tauri::command]
pub fn get_file_detail(
    state: State<'_, AppState>,
    repository_id: String,
    file_id: String,
) -> std::result::Result<queries::FileDetail, String> {
    with_db(&state, |c| {
        queries::file_detail(c, &repository_id, &file_id)
    })
    .map_err(err)
}

#[tauri::command]
pub fn get_graph(
    state: State<'_, AppState>,
    repository_id: String,
    level: String,
    focus: Option<String>,
) -> std::result::Result<queries::GraphView, String> {
    with_db(&state, |c| {
        queries::graph_view(c, &repository_id, &level, focus.as_deref(), 400)
    })
    .map_err(err)
}

#[tauri::command]
pub fn get_findings(
    state: State<'_, AppState>,
    repository_id: String,
) -> std::result::Result<Vec<Finding>, String> {
    with_db(&state, |c| queries::findings(c, &repository_id)).map_err(err)
}

#[tauri::command]
pub fn get_hotspots(
    state: State<'_, AppState>,
    repository_id: String,
    limit: u32,
) -> std::result::Result<Vec<MetricRecord>, String> {
    with_db(&state, |c| queries::hotspots(c, &repository_id, limit)).map_err(err)
}

#[tauri::command]
pub fn search_code(
    state: State<'_, AppState>,
    repository_id: String,
    query: String,
    limit: u32,
) -> std::result::Result<Vec<queries::SearchResult>, String> {
    with_db(&state, |c| {
        queries::search(c, &repository_id, &query, limit)
    })
    .map_err(err)
}

#[tauri::command]
pub fn get_modules(
    state: State<'_, AppState>,
    repository_id: String,
) -> std::result::Result<Vec<String>, String> {
    with_db(&state, |c| queries::modules(c, &repository_id)).map_err(err)
}

#[tauri::command]
pub fn get_change_impact(
    state: State<'_, AppState>,
    repository_id: String,
    file_id: String,
) -> std::result::Result<crate::core::graph::ChangeImpact, String> {
    with_db(&state, |c| {
        queries::change_impact(c, &repository_id, &file_id)
    })
    .map_err(err)
}

#[tauri::command]
pub fn read_file_snippet(
    state: State<'_, AppState>,
    repository_id: String,
    file_id: String,
    start_line: u32,
    end_line: u32,
) -> std::result::Result<String, String> {
    let path = with_db(&state, |c| {
        c.query_row(
            "SELECT path FROM files WHERE id = ?1 AND repository_id = ?2",
            rusqlite::params![file_id, repository_id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(CoreError::from)
    })
    .map_err(err)?
    .ok_or_else(|| "file not found".to_string())?;

    let root = with_db(&state, |c| {
        c.query_row(
            "SELECT root_path FROM repositories WHERE id = ?1",
            rusqlite::params![repository_id],
            |r| r.get::<_, String>(0),
        )
        .map_err(CoreError::from)
    })
    .map_err(err)?;

    let full = PathBuf::from(root).join(&path);
    let content = std::fs::read_to_string(&full).map_err(|e| e.to_string())?;
    let start = start_line.max(1) as usize;
    let end = if end_line == 0 {
        start + 40
    } else {
        end_line as usize
    };
    let lines: Vec<&str> = content.lines().collect();
    let out = lines
        .iter()
        .enumerate()
        .skip(start - 1)
        .take(end.saturating_sub(start - 1).max(1))
        .map(|(i, l)| format!("{:>5} | {}", i + 1, l))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(out)
}

#[tauri::command]
pub fn open_in_editor(path: String, line: u32) -> std::result::Result<(), String> {
    // Best-effort: open with the OS default behavior for the path.
    // We intentionally do not execute repository code.
    let target = if line > 0 {
        format!("{path}:{line}")
    } else {
        path
    };
    open::that(&target).map_err(|e| e.to_string())
}
