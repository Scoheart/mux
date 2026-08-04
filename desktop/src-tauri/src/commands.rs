use mux_core::application::assets::{
    AssetCommitRequest, AssetOperationPlan, ConsumptionInventory, ModelAdoptionCandidate,
    PlanDeleteCentralAssetRequest, PlanSetAgentConsumptionRequest, PlanSetAssetConsumersRequest,
    PlanUpdateAgentCapabilitiesRequest, PlanUpdateAgentConfigurationRequest,
    PlanUpdateCentralAssetRequest,
};
use mux_core::application::mcp::catalog::{
    read_registry, read_registry_all, user_override_keys, CatalogItem,
};
use mux_core::application::operations::{
    CancelOperationRequest, CommitOperationRequest, OperationCommitResult,
    OperationPlan as UnifiedOperationPlan, PlanOperationRequest,
};
use mux_core::application::skills::{
    GithubEndpoints, OperationPlan, PlanImportRequest, PlanRemoveRequest, PlanRepairRequest,
    PlanSkillAssetImportRequest, PlanSkillAssetInstallRequest, PlanUpdateRequest, SkillAgentView,
    SkillCommitRequest, SkillDetail, SkillError, SkillSourceInput, SkillSourceResolution,
    SkillsInventory, UpdateCheckOutcome,
};
use mux_core::application::{BackendStatus, MuxCore};
use mux_core::domain::error::{CoreError, CoreResult};
use mux_core::domain::types::RegistryEntry;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AssetCommandError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, Value>,
}

impl From<String> for AssetCommandError {
    fn from(error: String) -> Self {
        let error = core_error_from_legacy(error, "asset_operation_failed");
        Self {
            code: error.code,
            message: error.message,
            details: error.details,
        }
    }
}

fn core_error_from_legacy(error: String, fallback_code: &str) -> CoreError {
    let (code, message) = error
        .split_once(':')
        .filter(|(code, _)| {
            !code.is_empty()
                && code
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '_')
        })
        .map(|(code, message)| (code.trim(), message.trim()))
        .unwrap_or((fallback_code, error.as_str()));
    enrich_backend_error(CoreError::new(code, message))
}

fn enrich_backend_error(mut error: CoreError) -> CoreError {
    match MuxCore::backend_status() {
        BackendStatus::Starting if error.code == "backend_initializing" => {
            error
                .details
                .insert("backend_state".into(), Value::String("starting".into()));
        }
        BackendStatus::ReadOnly { stage, .. } if error.code == "recovery_required" => {
            error
                .details
                .insert("backend_state".into(), Value::String("read_only".into()));
            error.details.insert("stage".into(), Value::String(stage));
        }
        BackendStatus::MigrationReviewRequired {
            stage, review_hash, ..
        } if error.code == "migration_review_required" => {
            error.details.insert(
                "backend_state".into(),
                Value::String("migration_review_required".into()),
            );
            error.details.insert("stage".into(), Value::String(stage));
            error
                .details
                .insert("review_hash".into(), Value::String(review_hash));
        }
        BackendStatus::CapabilityUnavailable {
            capability,
            stage,
            code,
            ..
        } if error.code == code => {
            error.details.insert(
                "backend_state".into(),
                Value::String("capability_unavailable".into()),
            );
            error
                .details
                .insert("capability".into(), serde_json::json!(capability));
            error.details.insert("stage".into(), Value::String(stage));
        }
        _ => {}
    }
    error
}

async fn asset_blocking<T, F>(operation: F) -> Result<T, AssetCommandError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| AssetCommandError {
            code: "worker_failed".into(),
            message: "后台任务失败，请重试。".into(),
            details: BTreeMap::new(),
        })?
        .map_err(Into::into)
}

async fn core_blocking<T, F>(operation: F) -> CoreResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CoreResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| CoreError::new("worker_failed", "后台任务失败，请重试。"))?
}

#[tauri::command]
pub async fn get_workspace_snapshot(
) -> CoreResult<mux_core::application::workspace::WorkspaceSnapshot> {
    core_blocking(mux_core::application::MuxCore::snapshot).await
}

#[tauri::command]
pub fn get_backend_status() -> BackendStatus {
    MuxCore::backend_status()
}

#[tauri::command]
pub fn get_migration_review() -> Option<mux_core::application::bootstrap::MigrationReview> {
    MuxCore::migration_review()
}

#[tauri::command]
pub async fn resolve_migration(
    request: mux_core::application::bootstrap::ResolveMigrationRequest,
) -> CoreResult<mux_core::application::bootstrap::MigrationResolutionOutcome> {
    core_blocking(move || MuxCore::resolve_migration(request)).await
}

#[tauri::command]
pub async fn list_agent_capabilities(
) -> CoreResult<Vec<mux_core::application::agents::AgentCapabilityView>> {
    core_blocking(mux_core::application::agents::list_capabilities).await
}

#[tauri::command]
pub async fn plan_operation(request: PlanOperationRequest) -> CoreResult<UnifiedOperationPlan> {
    core_blocking(move || mux_core::application::MuxCore::plan(request)).await
}

#[tauri::command]
pub async fn commit_operation(
    request: CommitOperationRequest,
) -> CoreResult<OperationCommitResult> {
    core_blocking(move || mux_core::application::MuxCore::commit(request)).await
}

#[tauri::command]
pub async fn cancel_operation(request: CancelOperationRequest) -> CoreResult<()> {
    core_blocking(move || mux_core::application::MuxCore::cancel(request)).await
}

#[tauri::command]
pub async fn list_consumption_inventory() -> Result<ConsumptionInventory, AssetCommandError> {
    asset_blocking(mux_core::application::assets::list_inventory).await
}

#[tauri::command]
pub async fn list_model_adoption_candidates(
) -> Result<Vec<ModelAdoptionCandidate>, AssetCommandError> {
    asset_blocking(mux_core::application::assets::list_model_adoption_candidates).await
}

#[tauri::command]
pub async fn plan_set_agent_consumption(
    request: PlanSetAgentConsumptionRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_set_agent_consumption(request)).await
}

#[tauri::command]
pub async fn plan_set_mcp_enabled(
    request: mux_core::application::assets::PlanSetMcpEnabledRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_set_mcp_enabled(request)).await
}

#[tauri::command]
pub async fn plan_set_skill_enabled(
    request: mux_core::application::assets::PlanSetSkillEnabledRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_set_skill_enabled(request)).await
}

#[tauri::command]
pub async fn plan_set_model_enabled(
    request: mux_core::application::assets::PlanSetModelEnabledRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_set_model_enabled(request)).await
}

#[tauri::command]
pub async fn plan_set_active_model(
    request: mux_core::application::assets::PlanSetActiveModelRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_set_active_model(request)).await
}

#[tauri::command]
pub async fn plan_set_asset_consumers(
    request: PlanSetAssetConsumersRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_set_asset_consumers(request)).await
}

#[tauri::command]
pub async fn plan_update_agent_configuration(
    request: PlanUpdateAgentConfigurationRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_update_agent_configuration(request))
        .await
}

#[tauri::command]
pub async fn plan_update_agent_capabilities(
    request: PlanUpdateAgentCapabilitiesRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_update_agent_capabilities(request))
        .await
}

#[tauri::command]
pub async fn plan_update_central_asset(
    request: PlanUpdateCentralAssetRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_update_central_asset(request)).await
}

#[tauri::command]
pub async fn plan_delete_central_asset(
    request: PlanDeleteCentralAssetRequest,
) -> Result<AssetOperationPlan, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::plan_delete_central_asset(request)).await
}

#[tauri::command]
pub async fn commit_asset_operation(
    request: AssetCommitRequest,
) -> Result<ConsumptionInventory, AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::commit_asset_operation(request)).await
}

#[tauri::command]
pub async fn cancel_asset_operation(operation_id: String) -> Result<(), AssetCommandError> {
    asset_blocking(move || mux_core::application::assets::cancel_asset_operation(&operation_id))
        .await
}

// ── User-level Skills ────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillCommandError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub findings_hash: Option<String>,
}

impl From<SkillError> for SkillCommandError {
    fn from(error: SkillError) -> Self {
        let mut parts = error.into_command_parts();
        let embedded_gate_code = parts
            .message
            .split_once(':')
            .filter(|(code, _)| matches!(*code, "backend_initializing" | "recovery_required"))
            .map(|(code, message)| (code.to_string(), message.trim_start().to_string()));
        let (code, message) = embedded_gate_code
            .unwrap_or_else(|| (parts.code.to_string(), std::mem::take(&mut parts.message)));
        let core = enrich_backend_error(CoreError::new(code, &message));
        Self {
            code: core.code,
            message,
            details: core.details,
            retry_at: parts.retry_at,
            findings_hash: parts.findings_hash,
        }
    }
}

fn worker_error<T: std::fmt::Display>(_error: T) -> SkillCommandError {
    SkillCommandError {
        code: "worker_failed".into(),
        message: "后台任务失败，请重试。".into(),
        details: BTreeMap::new(),
        retry_at: None,
        findings_hash: None,
    }
}

fn dialog_path_error<T: std::fmt::Display>(_error: T) -> SkillCommandError {
    SkillCommandError {
        code: "invalid_local_folder".into(),
        message: "无法读取所选本地文件夹。".into(),
        details: BTreeMap::new(),
        retry_at: None,
        findings_hash: None,
    }
}

async fn skill_blocking<T, F>(operation: F) -> Result<T, SkillCommandError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, SkillError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(worker_error)?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn list_skills_inventory() -> Result<SkillsInventory, SkillCommandError> {
    skill_blocking(mux_core::application::skills::list_inventory).await
}

#[tauri::command]
pub async fn list_skill_agents() -> Result<Vec<SkillAgentView>, SkillCommandError> {
    skill_blocking(mux_core::application::skills::list_skill_agents).await
}

#[tauri::command]
pub async fn get_skill_detail(identity: String) -> Result<SkillDetail, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::get_skill_detail(&identity)).await
}

#[tauri::command]
pub async fn resolve_skill_source(
    value: String,
) -> Result<SkillSourceResolution, SkillCommandError> {
    skill_blocking(move || {
        mux_core::application::skills::resolve_source(
            SkillSourceInput::Github { value },
            GithubEndpoints::production(),
        )
    })
    .await
}

#[tauri::command]
pub async fn resolve_local_skill_source_dialog(
    app: tauri::AppHandle,
) -> Result<Option<SkillSourceResolution>, SkillCommandError> {
    use tauri_plugin_dialog::DialogExt;

    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(worker_error)?;
    let Some(path) = picked else {
        return Ok(None);
    };
    let path = path.into_path().map_err(dialog_path_error)?;
    let value = path
        .to_str()
        .ok_or_else(|| SkillCommandError {
            code: "invalid_local_folder".into(),
            message: "所选本地文件夹路径不是有效 UTF-8。".into(),
            details: BTreeMap::new(),
            retry_at: None,
            findings_hash: None,
        })?
        .to_owned();

    skill_blocking(move || {
        mux_core::application::skills::resolve_source(
            SkillSourceInput::Local { path: value },
            GithubEndpoints::production(),
        )
    })
    .await
    .map(Some)
}

#[tauri::command]
pub async fn resolve_archive_skill_source_dialog(
    app: tauri::AppHandle,
) -> Result<Option<SkillSourceResolution>, SkillCommandError> {
    use tauri_plugin_dialog::DialogExt;

    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Skill 压缩包", &["zip", "tar", "tgz", "gz"])
            .blocking_pick_file()
    })
    .await
    .map_err(worker_error)?;
    let Some(path) = picked else {
        return Ok(None);
    };
    let path = path.into_path().map_err(dialog_path_error)?;
    let value = path
        .to_str()
        .ok_or_else(|| SkillCommandError {
            code: "invalid_archive_file".into(),
            message: "所选压缩包路径不是有效 UTF-8。".into(),
            details: BTreeMap::new(),
            retry_at: None,
            findings_hash: None,
        })?
        .to_owned();

    skill_blocking(move || {
        mux_core::application::skills::resolve_source(
            SkillSourceInput::Archive { path: value },
            GithubEndpoints::production(),
        )
    })
    .await
    .map(Some)
}

#[tauri::command]
pub async fn plan_skill_asset_install(
    request: PlanSkillAssetInstallRequest,
) -> Result<OperationPlan, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::plan_asset_install(request)).await
}

#[tauri::command]
pub async fn commit_skill_install(
    request: SkillCommitRequest,
) -> Result<SkillsInventory, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::commit_install(request)).await
}

#[tauri::command]
pub async fn plan_skill_import(
    request: PlanImportRequest,
) -> Result<OperationPlan, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::plan_import(request)).await
}

#[tauri::command]
pub async fn plan_skill_asset_import(
    request: PlanSkillAssetImportRequest,
) -> Result<OperationPlan, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::plan_asset_import(request)).await
}

#[tauri::command]
pub async fn commit_skill_import(
    request: SkillCommitRequest,
) -> Result<SkillsInventory, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::commit_import(request)).await
}

#[tauri::command]
pub async fn plan_skill_update(
    request: PlanUpdateRequest,
) -> Result<OperationPlan, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::plan_update(request)).await
}

#[tauri::command]
pub async fn commit_skill_update(
    request: SkillCommitRequest,
) -> Result<SkillsInventory, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::commit_update(request)).await
}

#[tauri::command]
pub async fn plan_skill_remove(
    request: PlanRemoveRequest,
) -> Result<OperationPlan, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::plan_remove(request)).await
}

#[tauri::command]
pub async fn commit_skill_remove(
    request: SkillCommitRequest,
) -> Result<SkillsInventory, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::commit_remove(request)).await
}

#[tauri::command]
pub async fn commit_skill_assignment(
    request: SkillCommitRequest,
) -> Result<SkillsInventory, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::commit_assignment(request)).await
}

#[tauri::command]
pub async fn plan_skill_repair(
    request: PlanRepairRequest,
) -> Result<OperationPlan, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::plan_repair(request)).await
}

#[tauri::command]
pub async fn commit_skill_repair(
    request: SkillCommitRequest,
) -> Result<SkillsInventory, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::commit_repair(request)).await
}

#[tauri::command]
pub async fn check_skill_updates(manual: bool) -> Result<UpdateCheckOutcome, SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::check_updates(manual)).await
}

#[tauri::command]
pub async fn cancel_skill_operation(operation_id: String) -> Result<(), SkillCommandError> {
    skill_blocking(move || mux_core::application::skills::cancel_operation(&operation_id)).await
}

// ── Model endpoint profiles ─────────────────────────────────────────────

#[tauri::command]
pub fn list_model_profiles() -> Vec<mux_core::application::models::ModelProfileView> {
    mux_core::application::models::list_profiles()
}

#[tauri::command]
pub fn list_model_providers() -> &'static [mux_core::application::models::ModelProviderView] {
    mux_core::application::models::list_providers()
}

#[tauri::command]
pub fn list_model_provider_instances(
) -> Vec<mux_core::application::models::ModelProviderInstanceView> {
    mux_core::application::models::list_provider_instances()
}

#[tauri::command]
pub fn infer_model_provider(base_url: String) -> String {
    mux_core::application::models::infer_provider(&base_url)
}

#[tauri::command]
pub fn list_model_agents() -> Result<Vec<mux_core::application::models::ModelAgentView>, String> {
    mux_core::application::models::list_agent_capabilities()
}

#[tauri::command]
pub fn list_registry() -> Vec<RegistryEntry> {
    // Read user overrides from settings.registry merged over builtin — same source
    // scan_installed / apply_install resolve against, so the UI stays consistent.
    read_registry()
}

/// Every entry copy from all enabled sources (not deduped), each flagged with
/// whether it's the in-effect (winning) copy. Drives the Registry's "全部" /
/// per-source views that must show shadowed copies too.
#[tauri::command]
pub fn list_registry_all() -> Vec<CatalogItem> {
    read_registry_all()
}

/// Composite keys (`name::transport`) of registry entries that currently have a
/// user override.
#[tauri::command]
pub fn list_custom_registry_keys() -> Vec<String> {
    user_override_keys()
}

/// Parse a pasted config blob (JSON or TOML) and add every MCP server it contains
/// to the managed "manual" source. Returns the names that were added.
#[tauri::command]
pub fn import_pasted_config(text: String) -> CoreResult<Vec<String>> {
    let entries = mux_core::application::mcp::operations::parse_pasted_entries(&text)
        .map_err(|error| core_error_from_legacy(error, "invalid_config"))?;
    let mut names = Vec::new();
    for entry in entries {
        let existing_key = read_registry()
            .iter()
            .any(|candidate| candidate.key() == entry.key())
            .then(|| entry.key());
        let plan = mux_core::application::assets::plan_update_central_asset(
            PlanUpdateCentralAssetRequest {
                draft: mux_core::application::assets::CentralAssetDraft::Mcp {
                    existing_key,
                    entry: Box::new(entry),
                },
            },
        )
        .map_err(|error| core_error_from_legacy(error, "asset_operation_failed"))?;
        if !plan.can_commit || plan.requires_conflict_confirmation {
            let _ = mux_core::application::assets::cancel_asset_operation(&plan.operation_id);
            return Err(CoreError::new(
                "conflict_confirmation_required",
                "粘贴导入需要覆盖漂移配置；请在资源审查界面逐项处理",
            ));
        }
        names.extend(
            plan.central_changes
                .iter()
                .filter_map(|change| match &change.asset {
                    mux_core::application::assets::AssetRef::Mcp { key } => {
                        key.rsplit_once("::").map(|(name, _)| name.to_string())
                    }
                    _ => None,
                }),
        );
        mux_core::application::assets::commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
            conflict_confirmation: None,
        })
        .map_err(|error| core_error_from_legacy(error, "asset_operation_failed"))?;
    }
    names.sort();
    names.dedup();
    Ok(names)
}

// ── Catalog sources (subscribe remote / add local) ────────────────────────
// Orchestration lives in `mux_core::application::mcp::sources`; these are thin command wrappers.

use mux_core::application::mcp::sources::{self, SourceView};

#[tauri::command]
pub fn list_sources() -> Vec<SourceView> {
    sources::list_views()
}

#[tauri::command]
pub fn subscribe_source(url: String, name: Option<String>) -> CoreResult<SourceView> {
    sources::subscribe(url, name)
        .map_err(|error| core_error_from_legacy(error, "source_operation_failed"))
}

/// Add a local source from an explicit path.
#[tauri::command]
pub fn add_local_source(path: String, name: Option<String>) -> CoreResult<SourceView> {
    sources::add_local(path, name)
        .map_err(|error| core_error_from_legacy(error, "source_operation_failed"))
}

/// Open a native file picker and add the chosen file as a local source. Returns
/// `None` if the user cancels. Desktop-only (native dialog); delegates to core.
///
/// **Must not block the main thread.** Sync Tauri commands run on the main
/// thread, and `blocking_pick_file` there deadlocks: the panel needs the main
/// run loop to process the user's click, but the thread is parked waiting for
/// that very click → beachball/hang. So this is an `async` command (runs off the
/// main thread) and the blocking pick is pushed onto a worker via
/// `spawn_blocking`, leaving the main thread free to drive the panel.
#[tauri::command]
pub async fn add_local_source_dialog(app: tauri::AppHandle) -> CoreResult<Option<SourceView>> {
    use tauri_plugin_dialog::DialogExt;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("MCP 配置", &["json", "toml"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| CoreError::new("worker_failed", "后台任务失败，请重试。"))?;
    let Some(fp) = picked else { return Ok(None) };
    sources::add_local(fp.to_string(), None)
        .map(Some)
        .map_err(|error| core_error_from_legacy(error, "source_operation_failed"))
}

/// Export the complete effective catalog to a JSON file the user picks via a
/// native save dialog. Returns the written path, or `None` if the user cancels.
/// Async + `spawn_blocking` for the same main-thread reason as the picker above.
#[tauri::command]
pub async fn export_effective_dialog(app: tauri::AppHandle) -> CoreResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let content = mux_core::application::mcp::operations::export_effective()
        .map_err(|error| core_error_from_legacy(error, "export_failed"))?;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("MCP 配置", &["json"])
            .set_file_name("mux-effective.json")
            .blocking_save_file()
    })
    .await
    .map_err(|_| CoreError::new("worker_failed", "后台任务失败，请重试。"))?;
    let Some(fp) = picked else { return Ok(None) };
    let path = fp
        .into_path()
        .map_err(|error| CoreError::new("invalid_export_path", error.to_string()))?;
    MuxCore::independent_host_mutation("desktop_export", move || {
        std::fs::write(&path, content)
            .map_err(|error| CoreError::new("export_failed", error.to_string()))?;
        Ok(Some(path.display().to_string()))
    })
}

/// Add the bundled curated collection as an opt-in local source.
#[tauri::command]
pub fn add_builtin_collection() -> CoreResult<SourceView> {
    sources::add_official()
        .map_err(|error| core_error_from_legacy(error, "source_operation_failed"))
}

#[tauri::command]
pub fn refresh_source(id: String) -> CoreResult<SourceView> {
    sources::refresh(id).map_err(|error| core_error_from_legacy(error, "source_operation_failed"))
}

#[tauri::command]
pub fn set_source_enabled(id: String, enabled: bool) -> CoreResult<()> {
    sources::set_enabled(id, enabled)
        .map_err(|error| core_error_from_legacy(error, "source_operation_failed"))
}

#[tauri::command]
pub fn remove_source(id: String) -> CoreResult<()> {
    sources::remove(id).map_err(|error| core_error_from_legacy(error, "source_operation_failed"))
}

use mux_core::application::agents::{load_agents, AgentInfo};
use mux_core::domain::types::AgentDefinition;

/// 新增一个自定义 agent，持久化到 settings.agents（在内置/已有定义之上合并）。
/// id 为空或已存在时报错，避免误覆盖内置 agent。
#[tauri::command]
pub fn add_agent(id: String, def: AgentDefinition) -> CoreResult<()> {
    mux_core::application::agents::put(id, def, false)
        .map_err(|error| core_error_from_legacy(error, "agent_operation_failed"))
}

/// 编辑一个已存在 agent 的配置（路径 / 格式 / key），覆盖写回 settings.agents。
#[tauri::command]
pub fn update_agent(id: String, def: AgentDefinition) -> CoreResult<()> {
    mux_core::application::agents::put(id, def, true)
        .map_err(|error| core_error_from_legacy(error, "agent_operation_failed"))
}

#[tauri::command]
pub fn list_agents() -> Vec<AgentInfo> {
    mux_core::application::agents::list_infos()
}

#[tauri::command]
pub fn get_pinned_agents() -> Result<Vec<String>, String> {
    mux_core::application::ui::get_pinned_agents()
}

#[tauri::command]
pub fn set_pinned_agents(agent_ids: Vec<String>) -> CoreResult<Vec<String>> {
    mux_core::application::ui::set_pinned_agents(agent_ids)
        .map_err(|error| core_error_from_legacy(error, "ui_preference_failed"))
}

#[tauri::command]
pub fn get_ui_locale() -> Result<Option<String>, String> {
    mux_core::application::ui::get_ui_locale()
}

#[tauri::command]
pub fn set_ui_locale(locale: Option<String>) -> CoreResult<Option<String>> {
    mux_core::application::ui::set_ui_locale(locale)
        .map_err(|error| core_error_from_legacy(error, "ui_preference_failed"))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProxySettingsView {
    pub proxy_url: Option<String>,
}

#[tauri::command]
pub fn get_proxy_settings() -> Result<ProxySettingsView, String> {
    mux_core::application::network::get_proxy_settings().map(|settings| ProxySettingsView {
        proxy_url: settings.proxy_url,
    })
}

#[tauri::command]
pub fn set_proxy_settings(proxy_url: Option<String>) -> CoreResult<ProxySettingsView> {
    mux_core::application::network::set_proxy_url(proxy_url)
        .map(|settings| ProxySettingsView {
            proxy_url: settings.proxy_url,
        })
        .map_err(|error| core_error_from_legacy(error, "network_settings_failed"))
}

pub use mux_core::application::mcp::operations::InstalledMcp;

/// 扫描全局配置文件，返回「谁装在哪」。MUX 当前不管理项目级配置。
#[tauri::command]
pub fn scan_installed() -> Vec<InstalledMcp> {
    mux_core::application::mcp::operations::scan_installed(None)
}

use mux_core::application::mcp::operations::effective_config;
use mux_core::application::mcp::operations::{resolve_entry, target_file};
use mux_core::domain::mcp::OverridePatch;
use std::collections::HashMap;

#[derive(serde::Deserialize)]
pub struct PatchInput {
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}
impl PatchInput {
    fn to_patch(&self) -> OverridePatch {
        OverridePatch {
            args: self.args.clone(),
            env: self.env.clone(),
            url: self.url.clone(),
            headers: self.headers.clone(),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct InstallRequest {
    pub server_name: String,
    /// Transport variant to resolve ("stdio" | "http"). Defaults to stdio for
    /// older callers. The on-disk app config is still keyed by `server_name`.
    #[serde(default = "default_transport")]
    pub transport: String,
    pub agents: Vec<String>,
    #[serde(default)]
    pub overrides: HashMap<String, PatchInput>, // agentId -> patch
}

fn default_transport() -> String {
    "stdio".to_string()
}

#[derive(serde::Serialize)]
pub struct PlannedWrite {
    pub agent: String,
    pub file_path: String,
    pub config_json: String,
}

#[tauri::command]
pub fn preview_install(req: InstallRequest) -> Result<Vec<PlannedWrite>, String> {
    let entry = resolve_entry(&req.server_name, &req.transport)?;
    let agents = load_agents();
    let mut out = Vec::new();
    for agent_id in &req.agents {
        let def = agents
            .get(agent_id)
            .ok_or_else(|| format!("{agent_id}: unknown Agent"))?;
        if !mux_core::application::agents::supports_transport(agent_id, &req.transport) {
            return Err(format!(
                "{agent_id}: {} transport is not supported by this agent",
                req.transport
            ));
        }
        let path = target_file(def, "global", None)
            .ok_or_else(|| format!("{agent_id}: global config path is unavailable"))?;
        let patch = req.overrides.get(agent_id).map(|p| p.to_patch());
        let cfg = effective_config(&entry, patch.as_ref())
            .ok_or_else(|| format!("no config for {}", req.server_name))?;
        out.push(PlannedWrite {
            agent: agent_id.clone(),
            file_path: path.display().to_string(),
            config_json: serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mux_core::domain::types::{McpConfig, StdioConfig};

    #[test]
    fn preview_returns_planned_write_for_seeded_server() {
        use mux_core::domain::types::{RegistryConfig, RegistryEntry, StdioConfig};
        // No built-in catalog anymore: seed a manual entry through the real store
        // (a managed local source) in an isolated ~/.mux, then preview it.
        let _th = mux_core::testenv::TestHome::new("preview");
        let plan = mux_core::application::assets::plan_update_central_asset(
            PlanUpdateCentralAssetRequest {
                draft: mux_core::application::assets::CentralAssetDraft::Mcp {
                    existing_key: None,
                    entry: Box::new(RegistryEntry {
                        name: "seeded".into(),
                        description: String::new(),
                        tags: vec![],
                        config: RegistryConfig {
                            stdio: Some(StdioConfig {
                                command: "npx".into(),
                                args: Some(vec!["-y".into(), "seeded".into()]),
                                env: None,
                                cwd: None,
                            }),
                            http: None,
                        },
                        origin: None,
                        repo: None,
                    }),
                },
            },
        )
        .unwrap();
        mux_core::application::assets::commit_asset_operation(AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
            conflict_confirmation: None,
        })
        .unwrap();
        // Legacy callers may still send project fields. Serde ignores them and
        // the command resolves the global path unconditionally.
        let req: InstallRequest = serde_json::from_value(serde_json::json!({
            "server_name": "seeded",
            "transport": "stdio",
            "scope": "project",
            "project_dir": "/tmp/must-not-be-used",
            "agents": ["claude-code"],
            "overrides": {}
        }))
        .unwrap();
        let plan = preview_install(req).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].agent, "claude-code");
        assert!(plan[0].file_path.ends_with(".claude.json"));
        assert!(!plan[0].file_path.contains("must-not-be-used"));
        assert!(plan[0].config_json.contains("command"));
    }

    #[test]
    fn customized_comparison_uses_partial_eq() {
        // 验证 customized 比较逻辑：base != scanned.config → customized=true
        let base = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into(), "server".into()]),
            env: None,
            cwd: None,
        });
        let same = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into(), "server".into()]),
            env: None,
            cwd: None,
        });
        let modified = McpConfig::Stdio(StdioConfig {
            command: "npx".into(),
            args: Some(vec!["-y".into(), "server".into()]),
            env: Some(std::collections::HashMap::from([(
                "KEY".into(),
                "val".into(),
            )])),
            cwd: None,
        });
        // 未修改 → customized = false
        assert!(!(base != same));
        // 已修改 → customized = true
        assert!(base != modified);
    }
}
