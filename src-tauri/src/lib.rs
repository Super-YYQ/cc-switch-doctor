//! CC Switch Doctor library entrypoint.
//!
//! Hard security rules (enforced by CI scanners + code structure):
//! - Pure HTTP API tests only (reqwest).
//! - Never spawn processes / AI CLIs.
//! - Never read protected login dirs (.codex / .claude / opencode / .gemini) — FORBIDDEN_PATH_DOC_ONLY
//! - CC Switch DB is read-only.
//! - Full API keys stay in Rust memory only.

#![deny(unsafe_code)]

pub mod ccs_adapter;
pub mod commands;
pub mod diagnostics;
pub mod error;
pub mod protocols;
pub mod security;
pub mod state;
pub mod updates;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::discover_cc_switch,
            commands::scan_providers,
            commands::refresh_providers,
            commands::select_database,
            commands::start_diagnosis,
            commands::cancel_diagnosis,
            commands::check_updates,
            commands::get_app_info,
            commands::estimate_diagnosis,
        ])
        .setup(|app| {
            // Best-effort discovery at startup; failures are non-fatal.
            let state = app.state::<AppState>();
            let _ = state.discover_and_scan();
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppState>();
                state.cancel_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running CC Switch Doctor");
}
