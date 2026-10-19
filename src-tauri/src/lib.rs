mod commands;
pub mod core;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_app_database,
            commands::list_repositories,
            commands::open_repository,
            commands::get_summary,
            commands::start_index,
            commands::cancel_index,
            commands::list_files,
            commands::get_file_detail,
            commands::get_graph,
            commands::get_findings,
            commands::get_hotspots,
            commands::search_code,
            commands::get_modules,
            commands::get_change_impact,
            commands::read_file_snippet,
            commands::open_in_editor,
        ])
        .setup(|app| {
            let path = app
                .path()
                .app_data_dir()
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            std::fs::create_dir_all(&path)?;
            let conn = core::db::open(&path.join("archaeologist.db"))
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let state = app.state::<commands::AppState>();
            *state.db.lock().unwrap() = Some(conn);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
