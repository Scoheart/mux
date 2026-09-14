use mux_core::application::agent_launch::{self, LaunchInfo, LaunchReceipt, LaunchTarget};

#[tauri::command]
pub async fn get_agent_launch_info(agent_id: String) -> Result<LaunchInfo, String> {
    tauri::async_runtime::spawn_blocking(move || agent_launch::info(&agent_id)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn configure_agent_launch(agent_id: String, target: Option<LaunchTarget>) -> Result<LaunchInfo, String> {
    tauri::async_runtime::spawn_blocking(move || agent_launch::configure(&agent_id, target)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn launch_agent(agent_id: String, directory: Option<String>) -> Result<LaunchReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || agent_launch::launch(&agent_id, directory)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_terminal_settings() -> Result<mux_core::application::terminals::TerminalSettings, String> {
    tauri::async_runtime::spawn_blocking(mux_core::application::terminals::settings).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_terminal_preference(terminal_id: String) -> Result<mux_core::application::terminals::TerminalSettings, String> {
    tauri::async_runtime::spawn_blocking(move || mux_core::application::terminals::configure(&terminal_id)).await.map_err(|e| e.to_string())?
}
