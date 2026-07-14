pub mod cli_tool;
pub mod commands;
pub mod updater_guard;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Self-update: checks the stable-release channel (latest.json on the
        // newest vX.Y.Z GitHub Release); the frontend drives the UX.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Needed to relaunch the app after an update is installed.
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Fold any legacy ~/.mux files into a single settings.json on first run.
            mux_core::settings::migrate_if_needed();
            // Move any legacy settings.registry entries into the managed
            // "manual"/"discovered" local-source files (one-time).
            mux_core::registry::migrate_registry_to_sources();
            // Pre-detect each agent's existing MCP servers into the Registry so they
            // show up (and become manageable) the moment the app opens. Global scope
            // only here (no project dir at launch); best-effort.
            let _ = commands::import_discovered();

            // macOS may keep the process alive after the last window closes.
            // Always restore the configured main window on a fresh launch.
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
                window.set_focus()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_registry,
            commands::list_registry_all,
            commands::upsert_registry_entry,
            commands::delete_registry_entry,
            commands::list_custom_registry_keys,
            commands::import_pasted_config,
            commands::list_sources,
            commands::subscribe_source,
            commands::add_local_source,
            commands::add_local_source_dialog,
            commands::export_effective_dialog,
            commands::add_builtin_collection,
            commands::refresh_source,
            commands::set_source_enabled,
            commands::remove_source,
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
            commands::delete_mcp,
            commands::resync_entry,
            commands::forget_entry,
            cli_tool::cli_status,
            cli_tool::install_cli,
            updater_guard::update_environment
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
