use desktop_lib::commands;
use mux_core::consumption::{
    CentralAssetDraft, PlanUpdateCentralAssetRequest,
};
use mux_core::types::{RegistryConfig, RegistryEntry, StdioConfig};
use serde_json::json;

#[test]
fn lifecycle_requests_keep_the_tagged_core_wire_shape() {
    let request: PlanUpdateCentralAssetRequest = serde_json::from_value(json!({
        "draft": {
            "domain": "mcp",
            "entry": {
                "name": "local",
                "description": "",
                "tags": [],
                "config": {"stdio": {"command": "local-server"}}
            }
        }
    }))
    .unwrap();
    assert!(matches!(
        request.draft,
        CentralAssetDraft::Mcp {
            existing_key: None,
            ..
        }
    ));

    let unknown = serde_json::from_value::<PlanUpdateCentralAssetRequest>(json!({
        "draft": {
            "domain": "model",
            "profile": {
                "id": "work",
                "name": "Work",
                "protocol": "openai-responses",
                "base_url": "https://example.invalid",
                "model": "model"
            },
            "agent_ids": ["codex"]
        }
    }));
    assert!(unknown.is_err(), "central drafts must reject hidden Agent selection");
}

#[test]
fn lifecycle_command_returns_a_secret_free_review_plan() {
    let home = mux_core::testenv::TestHome::new("tauri-consumption-command");
    let plan = tauri::async_runtime::block_on(commands::plan_update_central_asset(
        PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Mcp {
                existing_key: None,
                entry: Box::new(RegistryEntry {
                    name: "local".into(),
                    description: String::new(),
                    tags: Vec::new(),
                    config: RegistryConfig {
                        stdio: Some(StdioConfig {
                            command: "private-command-sentinel".into(),
                            args: None,
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
    ))
    .unwrap();
    assert_eq!(plan.central_changes.len(), 1);
    let serialized = serde_json::to_string(&plan).unwrap();
    assert!(!serialized.contains("private-command-sentinel"));
    let persisted = std::fs::read_to_string(
        home.home
            .join(".mux/staging/consumption")
            .join(&plan.operation_id)
            .join("plan.json"),
    )
    .unwrap();
    assert!(!persisted.contains("private-command-sentinel"));
    tauri::async_runtime::block_on(commands::cancel_asset_operation(plan.operation_id)).unwrap();
}

#[test]
fn desktop_registers_only_the_planned_asset_mutation_surface() {
    let source = include_str!("../src/lib.rs");
    for command in [
        "commands::plan_set_agent_consumption",
        "commands::plan_set_asset_consumers",
        "commands::plan_update_central_asset",
        "commands::plan_delete_central_asset",
        "commands::commit_asset_operation",
        "commands::cancel_asset_operation",
    ] {
        assert!(source.contains(command), "missing registration: {command}");
    }
    for legacy in [
        "commands::apply_model_profile",
        "commands::save_model_profile",
        "commands::delete_model_profile",
        "commands::upsert_registry_entry",
        "commands::delete_registry_entry",
        "commands::resync_entry",
        "commands::forget_entry",
        "commands::apply_install",
        "commands::uninstall",
        "commands::disable_mcp",
        "commands::enable_mcp",
        "commands::delete_mcp",
        "commands::plan_skill_assignment",
    ] {
        assert!(!source.contains(legacy), "legacy Desktop mutation is registered: {legacy}");
    }
}
