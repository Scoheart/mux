pub mod core;
pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|_app| {
            // Fold any legacy ~/.mux files into a single settings.json on first run.
            core::settings::migrate_if_needed();
            // Pre-detect each agent's existing MCP servers into the Registry so they
            // show up (and become manageable) the moment the app opens. Global scope
            // only here (no project dir at launch); best-effort.
            let _ = commands::import_discovered(None);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_registry,
            commands::upsert_registry_entry,
            commands::delete_registry_entry,
            commands::list_custom_registry_keys,
            commands::list_agents,
            commands::add_agent,
            commands::update_agent,
            commands::scan_installed,
            commands::import_discovered,
            commands::preview_install,
            commands::apply_install,
            commands::uninstall,
            commands::disable_mcp,
            commands::enable_mcp,
            commands::delete_mcp
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
