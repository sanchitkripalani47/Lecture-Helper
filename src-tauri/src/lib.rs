pub mod batch;
pub mod commands;
pub mod discovery;
pub mod domain;
pub mod infrastructure;
pub mod processing;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(commands::BatchState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::discover_jobs,
            commands::start_batch,
            commands::cancel_batch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lecture Helper");
}
