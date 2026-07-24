#![cfg(unix)]

mod support;

use mux_core::consumption::{
    commit_asset_operation, list_consumption_inventory, plan_set_agent_consumption,
    AgentConsumptionSelection, AssetCommitRequest, AssetRef, ConsumptionStatus,
    McpConsumptionRecord, PlanSetAgentConsumptionRequest,
};
use mux_core::models::{apply_profile, save_profile};
use mux_core::ops::install;
use mux_core::r#override::OverridePatch;
use mux_core::registry::write_manual_entry;
use mux_core::settings::{mutate_settings, AgentConfigPathOverride};
use mux_core::testenv::TestHome;
use mux_core::types::{ModelProfile, ModelProtocol, RegistryConfig, RegistryEntry, StdioConfig};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use support::skills::SkillsFixture;

fn central_mcp() {
    write_manual_entry(&RegistryEntry {
        name: "local".into(),
        description: "Local fixture".into(),
        tags: Vec::new(),
        config: RegistryConfig {
            stdio: Some(StdioConfig {
                command: "local-server".into(),
                args: Some(vec!["serve".into()]),
                env: None,
                cwd: None,
            }),
            http: None,
        },
        origin: None,
        repo: None,
    })
    .unwrap();
}

#[test]
fn mcp_inventory_reconciles_desired_and_external_without_writes() {
    let home = TestHome::new("consume-mcp");
    central_mcp();
    install(
        "local",
        "stdio",
        "global",
        &["claude-code".into(), "codex".into()],
        None,
        &HashMap::new(),
    )
    .unwrap();
    mutate_settings(|settings| {
        settings.mcp_consumptions = Some(BTreeMap::from([(
            "claude-code".into(),
            BTreeMap::from([(
                "local::stdio".into(),
                McpConsumptionRecord {
                    asset_key: "local::stdio".into(),
                    enabled: true,
                    overrides: OverridePatch::default(),
                },
            )]),
        )]));
    })
    .unwrap();
    let settings_path = home.home.join(".mux/settings.json");
    let claude_path = home.home.join(".claude.json");
    let codex_path = home.home.join(".codex/config.toml");
    let before = [
        fs::read(&settings_path).unwrap(),
        fs::read(&claude_path).unwrap(),
        fs::read(&codex_path).unwrap(),
    ];

    let first = list_consumption_inventory().unwrap();
    let second = list_consumption_inventory().unwrap();
    assert_eq!(first, second);
    assert!(first.consumptions.iter().any(|item| {
        item.agent_id == "claude-code"
            && item.asset
                == (AssetRef::Mcp {
                    key: "local::stdio".into(),
                })
            && item.status == ConsumptionStatus::Synced
    }));
    assert!(first.external.iter().any(|item| {
        item.agent_id == "codex"
            && item.reason.as_deref() == Some("mcp_adoptable")
            && item.status == ConsumptionStatus::External
    }));
    assert_eq!(
        before,
        [
            fs::read(settings_path).unwrap(),
            fs::read(claude_path).unwrap(),
            fs::read(codex_path).unwrap(),
        ]
    );
}

#[test]
fn model_assignment_remains_visible_when_target_is_missing() {
    let _home = TestHome::new("consume-model");
    mutate_settings(|settings| {
        settings.model_profiles = Some(BTreeMap::from([(
            "inventory-profile".into(),
            ModelProfile {
                id: "inventory-profile".into(),
                name: "Inventory".into(),
                provider: "custom".into(),
                model_vendor: None,
                native_ids: Default::default(),
                protocol: ModelProtocol::AnthropicMessages,
                base_url: "https://example.invalid".into(),
                model: "example".into(),
                env_key: None,
                context_window: None,
                max_output_tokens: None,
                reasoning: Some(false),
            },
        )]));
        settings.model_assignments = Some(BTreeMap::from([(
            "claude-code".into(),
            "inventory-profile".into(),
        )]));
    })
    .unwrap();

    let inventory = list_consumption_inventory().unwrap();
    let model = inventory
        .consumptions
        .iter()
        .find(|item| matches!(&item.asset, AssetRef::Model { .. }))
        .unwrap();
    assert!(model.desired);
    assert!(!model.observed);
    assert_eq!(model.status, ConsumptionStatus::Drifted);
    assert_eq!(model.reason.as_deref(), Some("model_target_missing"));
}

fn model_profile() -> ModelProfile {
    ModelProfile {
        id: "inventory-profile".into(),
        name: "Inventory".into(),
        provider: "custom".into(),
        model_vendor: None,
        native_ids: Default::default(),
        protocol: ModelProtocol::OpenaiResponses,
        base_url: "https://example.invalid".into(),
        model: "example".into(),
        env_key: None,
        context_window: None,
        max_output_tokens: None,
        reasoning: Some(false),
    }
}

#[test]
fn unassigned_model_configuration_requires_confirmation_before_takeover() {
    let home = TestHome::new("consume-model-external");
    let target = home.home.join(".codex/config.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        &target,
        "model = \"external-model\"\nmodel_provider = \"external-provider\"\n",
    )
    .unwrap();
    mutate_settings(|settings| {
        settings.model_profiles = Some(BTreeMap::from([(
            "inventory-profile".into(),
            model_profile(),
        )]));
    })
    .unwrap();

    let inventory = list_consumption_inventory().unwrap();
    assert!(inventory.external.iter().any(|item| {
        item.agent_id == "codex"
            && matches!(&item.asset, AssetRef::Model { .. })
            && item.reason.as_deref() == Some("model_external_unmanaged")
    }));
    let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
        agent_id: "codex".into(),
        selection: AgentConsumptionSelection::Model {
            profile_ids: vec!["inventory-profile".into()],
        },
    })
    .unwrap();
    assert!(plan.can_commit);
    assert!(plan.requires_conflict_confirmation);
    assert!(plan
        .warnings
        .iter()
        .any(|warning| warning.contains("model_external_unmanaged")));
    let rejected = commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id.clone(),
        candidate_hash: plan.candidate_hash.clone(),
        conflict_confirmation: None,
    })
    .unwrap_err();
    assert!(rejected.starts_with("confirmation_required:"));
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "model = \"external-model\"\nmodel_provider = \"external-provider\"\n"
    );

    let inventory = commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id,
        candidate_hash: plan.candidate_hash.clone(),
        conflict_confirmation: Some(plan.candidate_hash),
    })
    .unwrap();
    assert!(inventory.consumptions.iter().any(|item| {
        item.agent_id == "codex"
            && item.asset
                == (AssetRef::Model {
                    profile_id: "inventory-profile".into(),
                })
            && item.status == ConsumptionStatus::Synced
    }));
    let updated = fs::read_to_string(target).unwrap();
    assert!(updated.contains("model = \"example\""));
    assert!(!updated.contains("external-model"));
}

#[test]
fn ambiguous_model_configuration_cannot_be_taken_over() {
    let home = TestHome::new("consume-model-ambiguous");
    let target = home.home.join(".codex/config.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        &target,
        "model = \"first\"\nmodel = \"second\"\nmodel_provider = \"external\"\n",
    )
    .unwrap();
    mutate_settings(|settings| {
        settings.model_profiles = Some(BTreeMap::from([(
            "inventory-profile".into(),
            model_profile(),
        )]));
    })
    .unwrap();

    let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
        agent_id: "codex".into(),
        selection: AgentConsumptionSelection::Model {
            profile_ids: vec!["inventory-profile".into()],
        },
    })
    .unwrap();
    assert!(!plan.can_commit);
    assert!(!plan.requires_conflict_confirmation);
    assert!(plan
        .warnings
        .iter()
        .any(|warning| warning.contains("model_external_conflicted")));
    assert_eq!(
        fs::read_to_string(target).unwrap(),
        "model = \"first\"\nmodel = \"second\"\nmodel_provider = \"external\"\n"
    );
}

#[test]
fn drifted_model_can_be_explicitly_reapplied_from_the_agent_plan() {
    let home = TestHome::new("consume-model-repair");
    save_profile(model_profile(), None).unwrap();
    apply_profile("codex", "inventory-profile").unwrap();
    let target = home.home.join(".codex/config.toml");
    let drifted = fs::read_to_string(&target)
        .unwrap()
        .replace("model = \"example\"", "model = \"tampered\"");
    fs::write(&target, drifted).unwrap();

    let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
        agent_id: "codex".into(),
        selection: AgentConsumptionSelection::Model {
            profile_ids: vec!["inventory-profile".into()],
        },
    })
    .unwrap();
    assert!(plan.can_commit);
    assert!(plan.requires_conflict_confirmation);
    assert!(plan
        .warnings
        .iter()
        .any(|warning| warning.contains("model_owned_fields_drift")));

    let inventory = commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id,
        candidate_hash: plan.candidate_hash.clone(),
        conflict_confirmation: Some(plan.candidate_hash),
    })
    .unwrap();
    assert!(inventory.consumptions.iter().any(|item| {
        item.agent_id == "codex"
            && item.status == ConsumptionStatus::Synced
            && item.asset
                == (AssetRef::Model {
                    profile_id: "inventory-profile".into(),
                })
    }));
    let repaired = fs::read_to_string(target).unwrap();
    assert!(repaired.contains("model = \"example\""));
    assert!(!repaired.contains("tampered"));
}

#[test]
fn model_plan_snapshots_and_writes_the_configured_override_path() {
    let home = TestHome::new("consume-model-custom-path");
    save_profile(model_profile(), None).unwrap();
    mutate_settings(|settings| {
        settings.agent_config_paths = Some(BTreeMap::from([(
            "codex".into(),
            AgentConfigPathOverride {
                model_paths: Some(vec!["~/.custom/codex-model.toml".into()]),
                skills_global_dir: None,
                ..Default::default()
            },
        )]));
    })
    .unwrap();

    let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
        agent_id: "codex".into(),
        selection: AgentConsumptionSelection::Model {
            profile_ids: vec!["inventory-profile".into()],
        },
    })
    .unwrap();
    assert_eq!(plan.target_files, ["~/.custom/codex-model.toml"]);
    let inventory = commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id,
        candidate_hash: plan.candidate_hash,
        conflict_confirmation: None,
    })
    .unwrap();
    assert!(inventory
        .consumptions
        .iter()
        .any(|item| { item.agent_id == "codex" && item.status == ConsumptionStatus::Synced }));
    assert!(home.home.join(".custom/codex-model.toml").is_file());
    assert!(!home.home.join(".codex/config.toml").exists());
}

#[test]
fn shared_skill_target_projects_every_affected_agent() {
    let fixture = SkillsFixture::managed_on_targets("review-changes", &["agents-user"]);
    let inventory = list_consumption_inventory().unwrap();
    let consumers: Vec<_> = inventory
        .consumptions
        .iter()
        .filter(|item| {
            item.asset
                == (AssetRef::Skill {
                    name: "review-changes".into(),
                })
        })
        .map(|item| (item.agent_id.as_str(), item.status.clone()))
        .collect();
    assert!(consumers.contains(&("codex", ConsumptionStatus::Synced)));
    assert!(consumers.contains(&("cursor", ConsumptionStatus::Synced)));
    assert!(consumers.contains(&("gemini", ConsumptionStatus::Synced)));

    fs::remove_file(fixture.target("agents-user", "review-changes")).unwrap();
    let drifted = list_consumption_inventory().unwrap();
    assert!(drifted.consumptions.iter().any(|item| {
        matches!(&item.asset, AssetRef::Skill { name } if name == "review-changes")
            && item.reason.as_deref() == Some("skill_target_missing")
    }));
}
