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
            // Agent MCP files are observed state only. Startup must never turn
            // external configuration into a central asset or desired relation.

            // Skills owns its nested journal; recover it first, then let the
            // cross-domain asset coordinator either finalize a verified commit
            // or restore its private rollback snapshots.
            let skills_recovery_ok = mux_core::skills::recover_pending().is_ok();
            let asset_recovery_ok =
                mux_core::consumption::recover_pending_asset_operations().is_ok();
            let recovery_ok = skills_recovery_ok && asset_recovery_ok;
            if recovery_ok {
                std::thread::spawn(|| {
                    let _ = mux_core::skills::check_updates_if_due();
                });
            }

            // macOS may keep the process alive after the last window closes.
            // Always restore the configured main window on a fresh launch.
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
                window.set_focus()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_consumption_inventory,
            commands::list_mcp_adoption_candidates,
            commands::plan_mcp_adoption,
            commands::plan_set_agent_consumption,
            commands::plan_set_mcp_enabled,
            commands::plan_set_asset_consumers,
            commands::plan_update_agent_configuration,
            commands::plan_update_central_asset,
            commands::plan_delete_central_asset,
            commands::commit_asset_operation,
            commands::cancel_asset_operation,
            commands::list_skills_inventory,
            commands::list_skill_migration_candidates,
            commands::list_skill_agents,
            commands::get_skill_detail,
            commands::resolve_skill_source,
            commands::resolve_local_skill_source_dialog,
            commands::resolve_archive_skill_source_dialog,
            commands::plan_skill_asset_install,
            commands::commit_skill_install,
            commands::plan_skill_asset_import,
            commands::plan_skill_import,
            commands::commit_skill_import,
            commands::plan_skill_update,
            commands::commit_skill_update,
            commands::plan_skill_remove,
            commands::commit_skill_remove,
            commands::commit_skill_assignment,
            commands::plan_skill_repair,
            commands::commit_skill_repair,
            commands::check_skill_updates,
            commands::cancel_skill_operation,
            commands::list_registry,
            commands::list_model_profiles,
            commands::list_model_agents,
            commands::list_registry_all,
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
            commands::get_pinned_agents,
            commands::set_pinned_agents,
            commands::get_proxy_settings,
            commands::set_proxy_settings,
            commands::add_agent,
            commands::update_agent,
            commands::scan_installed,
            commands::preview_install,
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
