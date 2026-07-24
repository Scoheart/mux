#![cfg(unix)]

mod support;

use mux_core::consumption::{
    commit_asset_operation, plan_delete_central_asset, plan_reapply_mcp, plan_set_active_model,
    plan_set_agent_consumption, plan_set_mcp_enabled, plan_set_model_enabled,
    plan_set_skill_enabled, plan_update_central_asset, AgentConsumptionSelection,
    AssetCommitRequest, AssetRef, CentralAssetDraft, PlanDeleteCentralAssetRequest,
    PlanReapplyMcpRequest, PlanSetActiveModelRequest, PlanSetAgentConsumptionRequest,
    PlanSetMcpEnabledRequest, PlanSetModelEnabledRequest, PlanSetSkillEnabledRequest,
    PlanUpdateCentralAssetRequest,
};
use mux_core::models::{apply_profile, list_profiles, reconcile_active_models, save_profile};
use mux_core::registry::{read_registry, write_manual_entry};
use mux_core::settings::load_settings;
use mux_core::testenv::TestHome;
use mux_core::types::{ModelProfile, ModelProtocol, RegistryConfig, RegistryEntry, StdioConfig};
use std::fs;
use support::skills::SkillsFixture;

fn commit(plan: mux_core::consumption::AssetOperationPlan) {
    commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id,
        candidate_hash: plan.candidate_hash,
        conflict_confirmation: None,
    })
    .unwrap();
}

fn mux_profile_id(profile_id: &str) -> String {
    format!(
        "mux_{}",
        profile_id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn mcp(command: &str) -> RegistryEntry {
    named_mcp("local", command)
}

fn named_mcp(name: &str, command: &str) -> RegistryEntry {
    RegistryEntry {
        name: name.into(),
        description: "Local fixture".into(),
        tags: vec!["test".into()],
        config: RegistryConfig {
            stdio: Some(StdioConfig {
                command: command.into(),
                args: Some(vec!["serve".into()]),
                env: None,
                cwd: None,
            }),
            http: None,
        },
        origin: None,
        repo: None,
    }
}

#[test]
fn unrelated_mcp_drift_does_not_block_or_get_overwritten_by_central_update() {
    let home = TestHome::new("central-mcp-unrelated-drift");
    write_manual_entry(&named_mcp("alpha", "alpha-old")).unwrap();
    write_manual_entry(&named_mcp("beta", "beta-old")).unwrap();
    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["alpha::stdio".into(), "beta::stdio".into()],
            },
        })
        .unwrap(),
    );
    let target = home.home.join(".claude.json");
    let customized = fs::read_to_string(&target)
        .unwrap()
        .replace("beta-old", "beta-custom");
    fs::write(&target, customized).unwrap();

    let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
        draft: CentralAssetDraft::Mcp {
            existing_key: Some("alpha::stdio".into()),
            entry: Box::new(named_mcp("alpha", "alpha-new")),
        },
    })
    .unwrap();
    assert!(plan.can_commit);
    assert!(!plan.requires_conflict_confirmation);
    assert!(plan.warnings.is_empty());
    commit(plan);

    let updated = fs::read_to_string(target).unwrap();
    assert!(updated.contains("alpha-new"));
    assert!(updated.contains("beta-custom"));
    assert!(!updated.contains("beta-old"));
}

#[test]
fn mcp_central_update_propagates_and_delete_cascades() {
    let home = TestHome::new("central-mcp-e2e");
    write_manual_entry(&mcp("old-server")).unwrap();
    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap(),
    );
    let target = home.home.join(".claude.json");
    assert!(fs::read_to_string(&target).unwrap().contains("old-server"));

    commit(
        plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Mcp {
                existing_key: Some("local::stdio".into()),
                entry: Box::new(mcp("new-server")),
            },
        })
        .unwrap(),
    );
    let updated = fs::read_to_string(&target).unwrap();
    assert!(updated.contains("new-server"));
    assert!(!updated.contains("old-server"));
    assert_eq!(
        load_settings().mcp_consumptions.unwrap()["claude-code"]
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["local::stdio"]
    );

    commit(
        plan_delete_central_asset(PlanDeleteCentralAssetRequest {
            asset: AssetRef::Mcp {
                key: "local::stdio".into(),
            },
            source_id: Some("manual".into()),
        })
        .unwrap(),
    );
    assert!(!read_registry()
        .iter()
        .any(|entry| entry.key() == "local::stdio"));
    assert!(!load_settings()
        .mcp_consumptions
        .unwrap_or_default()
        .contains_key("claude-code"));
    assert!(!fs::read_to_string(target).unwrap().contains("local"));
}

#[test]
fn central_mcp_create_does_not_touch_agent_targets() {
    let home = TestHome::new("central-mcp-create");
    let target = home.home.join(".claude.json");
    let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
        draft: CentralAssetDraft::Mcp {
            existing_key: None,
            entry: Box::new(mcp("central-only")),
        },
    })
    .unwrap();
    assert!(plan.affected_agent_ids.is_empty());
    commit(plan);
    assert!(!target.exists());
    assert!(load_settings().mcp_consumptions.is_none());
}

#[test]
fn drifted_consumer_requires_bound_confirmation_before_central_update() {
    let home = TestHome::new("central-mcp-drift");
    write_manual_entry(&mcp("old-server")).unwrap();
    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap(),
    );
    let target = home.home.join(".claude.json");
    let customized = fs::read_to_string(&target)
        .unwrap()
        .replace("old-server", "custom-server");
    fs::write(&target, customized).unwrap();

    let plan = plan_update_central_asset(PlanUpdateCentralAssetRequest {
        draft: CentralAssetDraft::Mcp {
            existing_key: Some("local::stdio".into()),
            entry: Box::new(mcp("new-server")),
        },
    })
    .unwrap();
    assert!(plan.can_commit);
    assert!(plan.requires_conflict_confirmation);
    let rejected = commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id.clone(),
        candidate_hash: plan.candidate_hash.clone(),
        conflict_confirmation: None,
    })
    .unwrap_err();
    assert!(rejected.starts_with("confirmation_required:"));
    assert_eq!(
        read_registry()
            .into_iter()
            .find(|entry| entry.key() == "local::stdio")
            .unwrap()
            .config
            .stdio
            .unwrap()
            .command,
        "old-server"
    );
    assert!(fs::read_to_string(&target)
        .unwrap()
        .contains("custom-server"));

    commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id,
        candidate_hash: plan.candidate_hash.clone(),
        conflict_confirmation: Some(plan.candidate_hash),
    })
    .unwrap();
    assert!(fs::read_to_string(target).unwrap().contains("new-server"));
}

#[test]
fn mcp_reapply_repairs_drift_without_changing_the_central_asset() {
    let home = TestHome::new("central-mcp-reapply");
    write_manual_entry(&mcp("managed-server")).unwrap();
    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap(),
    );
    let target = home.home.join(".claude.json");
    let drifted = fs::read_to_string(&target)
        .unwrap()
        .replace("managed-server", "tampered-server");
    fs::write(&target, drifted).unwrap();

    let plan = plan_reapply_mcp(PlanReapplyMcpRequest {
        asset_key: "local::stdio".into(),
    })
    .unwrap();
    assert!(plan.can_commit);
    assert!(plan.requires_conflict_confirmation);
    assert_eq!(
        plan.central_changes[0].summary,
        vec![
            "重新同步 MCP 配置",
            "将更新 1 个已关联 Agent",
            "中央配置保持不变",
        ]
    );
    commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id,
        candidate_hash: plan.candidate_hash.clone(),
        conflict_confirmation: Some(plan.candidate_hash),
    })
    .unwrap();

    let repaired = fs::read_to_string(target).unwrap();
    assert!(repaired.contains("managed-server"));
    assert!(!repaired.contains("tampered-server"));
    assert_eq!(
        read_registry()
            .into_iter()
            .find(|entry| entry.key() == "local::stdio")
            .unwrap()
            .config
            .stdio
            .unwrap()
            .command,
        "managed-server"
    );
}

#[test]
fn mcp_reapply_preserves_a_disabled_desired_relationship() {
    let home = TestHome::new("central-mcp-reapply-disabled");
    write_manual_entry(&mcp("managed-server")).unwrap();
    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap(),
    );
    commit(
        plan_set_mcp_enabled(PlanSetMcpEnabledRequest {
            agent_id: "claude-code".into(),
            asset_key: "local::stdio".into(),
            enabled: false,
        })
        .unwrap(),
    );

    let target = home.home.join(".claude.json");
    assert!(!fs::read_to_string(&target)
        .unwrap()
        .contains("managed-server"));

    commit(
        plan_reapply_mcp(PlanReapplyMcpRequest {
            asset_key: "local::stdio".into(),
        })
        .unwrap(),
    );

    assert!(!fs::read_to_string(target)
        .unwrap()
        .contains("managed-server"));
    assert!(
        !load_settings().mcp_consumptions.unwrap()["claude-code"]["local::stdio"].enabled,
        "reapply must refresh the disabled snapshot without enabling the relationship"
    );
}

#[test]
fn mcp_reapply_rejects_a_catalog_change_after_review() {
    let home = TestHome::new("central-mcp-reapply-stale-catalog");
    write_manual_entry(&mcp("reviewed-server")).unwrap();
    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "claude-code".into(),
            selection: AgentConsumptionSelection::Mcp {
                asset_keys: vec!["local::stdio".into()],
            },
        })
        .unwrap(),
    );
    let target = home.home.join(".claude.json");
    let drifted = fs::read_to_string(&target)
        .unwrap()
        .replace("reviewed-server", "local-customization");
    fs::write(&target, &drifted).unwrap();

    let plan = plan_reapply_mcp(PlanReapplyMcpRequest {
        asset_key: "local::stdio".into(),
    })
    .unwrap();
    write_manual_entry(&mcp("changed-after-review")).unwrap();
    let error = commit_asset_operation(AssetCommitRequest {
        operation_id: plan.operation_id,
        candidate_hash: plan.candidate_hash.clone(),
        conflict_confirmation: Some(plan.candidate_hash),
    })
    .unwrap_err();

    assert!(
        error.contains("central MCP catalog changed after review"),
        "{error}"
    );
    assert_eq!(fs::read_to_string(target).unwrap(), drifted);
}

fn model(model: &str) -> ModelProfile {
    ModelProfile {
        id: "work".into(),
        name: "Work".into(),
        provider: "custom".into(),
        model_vendor: None,
        native_ids: Default::default(),
        protocol: ModelProtocol::OpenaiResponses,
        base_url: "https://example.invalid/v1".into(),
        model: model.into(),
        env_key: None,
        context_window: Some(128_000),
        max_output_tokens: Some(8_192),
        reasoning: true,
    }
}

#[test]
fn model_edit_propagates_without_dropping_assignment_and_delete_cascades() {
    let home = TestHome::new("central-model-e2e");
    save_profile(model("old-model"), None).unwrap();
    apply_profile("codex", "work").unwrap();
    let target = home.home.join(".codex/config.toml");
    assert!(fs::read_to_string(&target).unwrap().contains("old-model"));

    commit(
        plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Model {
                existing_id: Some("work".into()),
                profile: Box::new(model("new-model")),
                credential: None,
            },
        })
        .unwrap(),
    );
    let updated = fs::read_to_string(&target).unwrap();
    assert!(updated.contains("new-model"));
    assert!(!updated.contains("old-model"));
    assert_eq!(load_settings().model_assignments.unwrap()["codex"], "work");

    commit(
        plan_delete_central_asset(PlanDeleteCentralAssetRequest {
            asset: AssetRef::Model {
                profile_id: "work".into(),
            },
            source_id: None,
        })
        .unwrap(),
    );
    assert!(list_profiles().is_empty());
    assert!(!load_settings()
        .model_assignments
        .unwrap_or_default()
        .contains_key("codex"));
    let cleared = fs::read_to_string(target).unwrap();
    assert!(!cleared.contains("work"));
    assert!(!cleared.contains("new-model"));
}

#[test]
fn grok_build_consumes_and_switches_central_profiles() {
    let home = TestHome::new("central-model-grok-build");
    let mut responses = model("gpt-custom");
    responses.id = "openai-work".into();
    responses.env_key = Some("OPENAI_WORK_API_KEY".into());
    let mut messages = model("claude-custom");
    messages.id = "anthropic-work".into();
    messages.protocol = ModelProtocol::AnthropicMessages;
    messages.env_key = Some("ANTHROPIC_WORK_API_KEY".into());
    save_profile(responses.clone(), None).unwrap();
    save_profile(messages.clone(), None).unwrap();

    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "grok-build".into(),
            selection: AgentConsumptionSelection::Model {
                profile_ids: vec![responses.id.clone()],
            },
        })
        .unwrap(),
    );
    let target = home.home.join(".grok/config.toml");
    let first = fs::read_to_string(&target).unwrap();
    assert!(first.contains("api_backend = \"responses\""));
    assert!(first.contains("env_key = \"OPENAI_WORK_API_KEY\""));

    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "grok-build".into(),
            selection: AgentConsumptionSelection::Model {
                profile_ids: vec![messages.id.clone()],
            },
        })
        .unwrap(),
    );
    let switched = fs::read_to_string(target).unwrap();
    assert!(switched.contains("api_backend = \"messages\""));
    assert!(switched.contains("env_key = \"ANTHROPIC_WORK_API_KEY\""));
    assert!(!switched.contains("OPENAI_WORK_API_KEY"));
    assert_eq!(
        load_settings().model_assignments.unwrap()["grok-build"],
        messages.id
    );
}

#[test]
fn grok_build_delete_preserves_an_unmanaged_model_without_failing_verification() {
    let home = TestHome::new("central-model-grok-build-delete-with-external");
    let target = home.home.join(".grok/config.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        &target,
        "[models]\ndefault = \"private\"\n\n[model.private]\nmodel = \"keep\"\n",
    )
    .unwrap();

    let mut profile = model("delete-me");
    profile.id = "a".into();
    profile.env_key = Some("DELETE_ME_API_KEY".into());
    save_profile(profile.clone(), None).unwrap();
    apply_profile("grok-build", &profile.id).unwrap();

    commit(
        plan_delete_central_asset(PlanDeleteCentralAssetRequest {
            asset: AssetRef::Model {
                profile_id: profile.id.clone(),
            },
            source_id: None,
        })
        .unwrap(),
    );

    let cleared = fs::read_to_string(target).unwrap();
    assert!(cleared.contains("model.private"));
    assert!(cleared.contains("model = \"keep\""));
    assert!(!cleared.contains(&mux_profile_id(&profile.id)));
    assert!(list_profiles().is_empty());
}

#[test]
fn grok_build_keeps_multiple_profiles_and_falls_back_when_current_is_disabled() {
    let home = TestHome::new("central-model-grok-build-multiple");
    let mut first = model("first-model");
    first.id = "first".into();
    first.env_key = Some("FIRST_API_KEY".into());
    let mut second = model("second-model");
    second.id = "second".into();
    second.env_key = Some("SECOND_API_KEY".into());
    save_profile(first.clone(), None).unwrap();
    save_profile(second.clone(), None).unwrap();

    commit(
        plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
            agent_id: "grok-build".into(),
            selection: AgentConsumptionSelection::Model {
                profile_ids: vec![first.id.clone(), second.id.clone()],
            },
        })
        .unwrap(),
    );
    let initial = load_settings().model_selection("grok-build");
    assert_eq!(initial.profiles.len(), 2);
    let initial_active = initial.active_profile_id.unwrap();
    let switched = if initial_active == first.id {
        second.id.clone()
    } else {
        first.id.clone()
    };
    commit(
        plan_set_active_model(PlanSetActiveModelRequest {
            agent_id: "grok-build".into(),
            profile_id: switched.clone(),
        })
        .unwrap(),
    );

    commit(
        plan_set_model_enabled(PlanSetModelEnabledRequest {
            agent_id: "grok-build".into(),
            profile_id: switched.clone(),
            enabled: false,
        })
        .unwrap(),
    );
    let disabled = load_settings().model_selection("grok-build");
    assert_eq!(
        disabled.active_profile_id.as_deref(),
        Some(initial_active.as_str())
    );
    assert!(!disabled.profiles[&switched].enabled);
    assert!(disabled.profiles[&initial_active].enabled);
    let target = home.home.join(".grok/config.toml");
    let disabled_config = fs::read_to_string(&target).unwrap();
    let removed_env = if switched == first.id {
        "FIRST_API_KEY"
    } else {
        "SECOND_API_KEY"
    };
    assert!(!disabled_config.contains(removed_env));

    let switch_plan = plan_set_active_model(PlanSetActiveModelRequest {
        agent_id: "grok-build".into(),
        profile_id: switched.clone(),
    })
    .unwrap();
    assert!(switch_plan.model_state_changes.iter().any(|change| {
        change.profile_id == switched
            && change.after.enabled
            && change.after.active
            && !change.before.enabled
            && !change.before.active
    }));
    commit(switch_plan);
    let reenabled = load_settings().model_selection("grok-build");
    assert_eq!(
        reenabled.active_profile_id.as_deref(),
        Some(switched.as_str())
    );
    assert!(reenabled.profiles[&switched].enabled);
    assert!(reenabled.profiles[&initial_active].enabled);
    let reenabled_config = fs::read_to_string(&target).unwrap();
    assert!(reenabled_config.contains("FIRST_API_KEY"));
    assert!(reenabled_config.contains("SECOND_API_KEY"));

    let native = fs::read_to_string(&target).unwrap();
    let switched_marker = format!("default = \"{}\"", mux_profile_id(&switched));
    let initial_marker = format!("default = \"{}\"", mux_profile_id(&initial_active));
    assert!(native.contains(&switched_marker));
    fs::write(&target, native.replace(&switched_marker, &initial_marker)).unwrap();
    reconcile_active_models().unwrap();
    assert_eq!(
        load_settings()
            .model_selection("grok-build")
            .active_profile_id,
        Some(initial_active)
    );
}

#[test]
fn shared_skill_target_expands_agent_intent_and_rejects_partial_asset_selection() {
    let _fixture = SkillsFixture::managed("review-changes");
    let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
        agent_id: "codex".into(),
        selection: AgentConsumptionSelection::Skill {
            names: vec!["review-changes".into()],
        },
    })
    .unwrap();
    assert_eq!(
        plan.affected_agent_ids,
        vec!["codex", "copilot-cli", "cursor", "gemini", "opencode"]
    );
    assert_eq!(plan.target_files, vec!["~/.agents/skills/review-changes"]);
    commit(plan);
    assert_eq!(
        load_settings().skill_assignments.unwrap()["review-changes"],
        std::collections::BTreeSet::from(["agents-user".into()])
    );

    let error = mux_core::consumption::plan_set_asset_consumers(
        mux_core::consumption::PlanSetAssetConsumersRequest {
            asset: AssetRef::Skill {
                name: "review-changes".into(),
            },
            agent_ids: vec!["codex".into()],
        },
    )
    .unwrap_err();
    assert!(error.starts_with("skill_shared_target_conflict:"));
}

#[test]
fn shared_skill_toggle_preserves_assignment_and_changes_the_physical_target_once() {
    let fixture = SkillsFixture::managed_on_targets("review-changes", &["agents-user"]);
    let target = fixture.target("agents-user", "review-changes");

    let disable = plan_set_skill_enabled(PlanSetSkillEnabledRequest {
        agent_id: "codex".into(),
        name: "review-changes".into(),
        enabled: false,
    })
    .unwrap();
    assert_eq!(
        disable.affected_agent_ids,
        vec!["codex", "copilot-cli", "cursor", "gemini", "opencode"]
    );
    assert_eq!(
        disable.target_files,
        vec!["~/.agents/skills/review-changes"]
    );
    commit(disable);

    let settings = load_settings();
    assert!(settings.skill_assignments.as_ref().unwrap()["review-changes"].contains("agents-user"));
    assert!(
        !settings.skill_consumptions.as_ref().unwrap()["review-changes"]["agents-user"].enabled
    );
    assert!(!target.exists());
    let disabled = mux_core::consumption::list_consumption_inventory().unwrap();
    let rows: Vec<_> = disabled
        .consumptions
        .iter()
        .filter(|item| {
            item.asset
                == (AssetRef::Skill {
                    name: "review-changes".into(),
                })
        })
        .collect();
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().all(|item| {
        item.desired
            && !item.observed
            && item.enabled == Some(false)
            && item.status == mux_core::consumption::ConsumptionStatus::Synced
    }));
    let repair = mux_core::skills::plan_repair(mux_core::skills::PlanRepairRequest {
        skill_name: "review-changes".into(),
        repair: mux_core::skills::RepairKind::Target {
            target_id: "agents-user".into(),
        },
    })
    .unwrap_err();
    assert!(matches!(
        repair,
        mux_core::skills::SkillError::Conflict { .. }
    ));

    commit(
        plan_set_skill_enabled(PlanSetSkillEnabledRequest {
            agent_id: "cursor".into(),
            name: "review-changes".into(),
            enabled: true,
        })
        .unwrap(),
    );
    let settings = load_settings();
    assert!(settings.skill_assignments.as_ref().unwrap()["review-changes"].contains("agents-user"));
    assert!(settings.skill_consumptions.as_ref().unwrap()["review-changes"]["agents-user"].enabled);
    assert!(target.is_symlink());
    let enabled = mux_core::consumption::list_consumption_inventory().unwrap();
    assert!(enabled
        .consumptions
        .iter()
        .filter(|item| item.asset
            == (AssetRef::Skill {
                name: "review-changes".into(),
            }))
        .all(|item| {
            item.observed
                && item.enabled == Some(true)
                && item.status == mux_core::consumption::ConsumptionStatus::Synced
        }));
}

#[test]
fn claude_skill_plan_reports_one_write_target_and_opencode_as_affected() {
    let _fixture = SkillsFixture::managed("frontend-design");
    let plan = plan_set_agent_consumption(PlanSetAgentConsumptionRequest {
        agent_id: "claude-code".into(),
        selection: AgentConsumptionSelection::Skill {
            names: vec!["frontend-design".into()],
        },
    })
    .unwrap();

    assert_eq!(plan.affected_agent_ids, vec!["claude-code", "opencode"]);
    assert_eq!(plan.target_files, vec!["~/.claude/skills/frontend-design"]);

    commit(plan);
    assert_eq!(
        load_settings().skill_assignments.unwrap()["frontend-design"],
        std::collections::BTreeSet::from(["claude-user".into()])
    );
}
