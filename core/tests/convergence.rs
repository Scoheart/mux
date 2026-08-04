#![cfg(unix)]

mod support;

use mux_core::application::operations::{
    CommitOperationRequest, OperationCommitResult, OperationPlan, PlanOperationRequest,
};
use mux_core::application::MuxCore;
use mux_core::domain::assets::{
    AgentConsumptionSelection, AssetCapability, AssetCommitRequest, AssetRef, ConsumptionInventory,
    ConsumptionStatus, ConvergenceAction, PlanConvergeConsumptionRequest,
    PlanSetAgentConsumptionRequest,
};
use mux_core::resources::mcp::registry::{read_registry, write_manual_entry};
use mux_core::resources::model::save_profile;
use mux_core::testenv::TestHome;
use mux_core::types::{ModelProfile, ModelProtocol, RegistryConfig, RegistryEntry, StdioConfig};
use std::fs;
use std::os::unix::fs::symlink;
use support::skills::{assert_managed_link, SkillsFixture};

fn mcp(command: &str) -> RegistryEntry {
    RegistryEntry {
        name: "local".into(),
        description: "convergence fixture".into(),
        tags: Vec::new(),
        config: RegistryConfig {
            stdio: Some(StdioConfig {
                command: command.into(),
                args: None,
                env: None,
                cwd: None,
            }),
            http: None,
        },
        origin: None,
        repo: None,
    }
}

fn model() -> ModelProfile {
    ModelProfile {
        id: "convergence-model".into(),
        provider_id: Some("convergence-provider".into()),
        name: "Convergence Model".into(),
        provider: "custom".into(),
        model_vendor: None,
        native_ids: Default::default(),
        protocol: ModelProtocol::OpenaiResponses,
        base_url: "https://example.invalid/v1".into(),
        endpoint_path: String::new(),
        model: "desired-model".into(),
        env_key: None,
        context_window: None,
        max_output_tokens: None,
        reasoning: Some(false),
    }
}

fn commit(plan: OperationPlan) -> ConsumptionInventory {
    let OperationPlan::Asset { plan } = plan else {
        panic!("expected an Asset convergence plan")
    };
    let result = MuxCore::commit(CommitOperationRequest::Asset {
        request: AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
        },
    })
    .unwrap();
    let OperationCommitResult::Asset { inventory } = result else {
        panic!("expected an Asset convergence result")
    };
    inventory
}

fn converge(
    inventory: &ConsumptionInventory,
    agent_id: &str,
    asset: AssetRef,
    action: ConvergenceAction,
) -> Result<OperationPlan, mux_core::domain::error::CoreError> {
    MuxCore::plan(PlanOperationRequest::ConvergeConsumption(
        PlanConvergeConsumptionRequest {
            agent_id: agent_id.into(),
            asset,
            action,
            observed_revision: inventory.revision.clone(),
        },
    ))
}

#[test]
fn mcp_adopt_restore_and_detach_are_exact_and_preserve_reviewed_agent_bytes() {
    let home = TestHome::new("unified-mcp-convergence");
    write_manual_entry(&mcp("desired-server")).unwrap();
    commit(
        MuxCore::plan(PlanOperationRequest::SetAgentConsumption(
            PlanSetAgentConsumptionRequest {
                agent_id: "claude-code".into(),
                selection: AgentConsumptionSelection::Mcp {
                    asset_keys: vec!["local::stdio".into()],
                },
            },
        ))
        .unwrap(),
    );
    let target = home.home.join(".claude.json");

    let adopted_bytes = fs::read_to_string(&target)
        .unwrap()
        .replace("desired-server", "agent-adopted-server");
    fs::write(&target, &adopted_bytes).unwrap();
    let observed = mux_core::application::assets::list_inventory().unwrap();
    let row = observed
        .consumptions
        .iter()
        .find(|row| {
            row.agent_id == "claude-code"
                && row.asset
                    == (AssetRef::Mcp {
                        key: "local::stdio".into(),
                    })
        })
        .unwrap();
    assert_eq!(row.status, ConsumptionStatus::ExternalChanged);
    assert_eq!(
        row.available_actions,
        vec![
            ConvergenceAction::AdoptObserved,
            ConvergenceAction::RestoreDesired,
            ConvergenceAction::Detach,
        ]
    );
    let inventory = commit(
        converge(
            &observed,
            "claude-code",
            row.asset.clone(),
            ConvergenceAction::AdoptObserved,
        )
        .unwrap(),
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), adopted_bytes);
    assert!(inventory.consumptions.iter().any(|row| {
        row.agent_id == "claude-code"
            && row.status == ConsumptionStatus::Synced
            && row.asset
                == (AssetRef::Mcp {
                    key: "local::stdio".into(),
                })
    }));
    assert_eq!(
        read_registry()
            .into_iter()
            .find(|entry| entry.key() == "local::stdio")
            .unwrap()
            .config
            .stdio
            .unwrap()
            .command,
        "agent-adopted-server"
    );

    let restore_drift = adopted_bytes.replace("agent-adopted-server", "restore-drift");
    fs::write(&target, &restore_drift).unwrap();
    let observed = mux_core::application::assets::list_inventory().unwrap();
    let inventory = commit(
        converge(
            &observed,
            "claude-code",
            AssetRef::Mcp {
                key: "local::stdio".into(),
            },
            ConvergenceAction::RestoreDesired,
        )
        .unwrap(),
    );
    assert!(fs::read_to_string(&target)
        .unwrap()
        .contains("agent-adopted-server"));
    assert!(inventory
        .consumptions
        .iter()
        .all(|row| row.status != ConsumptionStatus::ExternalChanged));

    let detached_bytes = fs::read_to_string(&target)
        .unwrap()
        .replace("agent-adopted-server", "detached-agent-server");
    fs::write(&target, &detached_bytes).unwrap();
    let observed = mux_core::application::assets::list_inventory().unwrap();
    let inventory = commit(
        converge(
            &observed,
            "claude-code",
            AssetRef::Mcp {
                key: "local::stdio".into(),
            },
            ConvergenceAction::Detach,
        )
        .unwrap(),
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), detached_bytes);
    assert!(!inventory.consumptions.iter().any(|row| {
        row.agent_id == "claude-code"
            && row.asset
                == (AssetRef::Mcp {
                    key: "local::stdio".into(),
                })
    }));
    assert!(inventory.external.iter().any(|row| {
        row.agent_id == "claude-code"
            && row.status == ConsumptionStatus::ExternalAdded
            && row.asset
                == (AssetRef::Mcp {
                    key: "local::stdio".into(),
                })
    }));
}

#[test]
fn convergence_rejects_a_stale_observation_revision() {
    let home = TestHome::new("stale-convergence-observation");
    write_manual_entry(&mcp("desired-server")).unwrap();
    commit(
        MuxCore::plan(PlanOperationRequest::SetAgentConsumption(
            PlanSetAgentConsumptionRequest {
                agent_id: "claude-code".into(),
                selection: AgentConsumptionSelection::Mcp {
                    asset_keys: vec!["local::stdio".into()],
                },
            },
        ))
        .unwrap(),
    );
    let target = home.home.join(".claude.json");
    let drifted = fs::read_to_string(&target)
        .unwrap()
        .replace("desired-server", "first-drift");
    fs::write(&target, drifted).unwrap();
    let stale = mux_core::application::assets::list_inventory().unwrap();
    let changed_again = fs::read_to_string(&target)
        .unwrap()
        .replace("first-drift", "second-drift");
    fs::write(&target, &changed_again).unwrap();

    let error = converge(
        &stale,
        "claude-code",
        AssetRef::Mcp {
            key: "local::stdio".into(),
        },
        ConvergenceAction::RestoreDesired,
    )
    .unwrap_err();
    assert_eq!(error.code, "observation_stale");
    assert_eq!(fs::read_to_string(target).unwrap(), changed_again);
}

#[test]
fn external_model_adoption_is_bound_to_one_exact_candidate() {
    let home = TestHome::new("exact-external-model-convergence");
    let target = home.home.join(".config/opencode/opencode.json");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        &target,
        r#"{
  "model": "legacy/active",
  "provider": {
    "legacy": {
      "npm": "@ai-sdk/openai-compatible",
      "options": {"baseURL": "https://openrouter.ai/api/v1", "apiKey": "{env:OPENROUTER_API_KEY}"},
      "models": {"active": {"name": "Active"}, "inactive": {"name": "Inactive"}}
    }
  }
}"#,
    )
    .unwrap();
    let reviewed_bytes = fs::read(&target).unwrap();
    let observed = mux_core::application::assets::list_inventory().unwrap();
    let external_models = observed
        .external
        .iter()
        .filter(|row| row.agent_id == "opencode" && matches!(&row.asset, AssetRef::Model { .. }))
        .collect::<Vec<_>>();
    assert_eq!(external_models.len(), 2);
    let adoptable = external_models
        .iter()
        .find(|row| {
            row.available_actions
                .contains(&ConvergenceAction::AdoptObserved)
        })
        .unwrap();
    let adopted_asset = adoptable.asset.clone();
    let other_asset = external_models
        .iter()
        .find(|row| row.asset != adopted_asset)
        .unwrap()
        .asset
        .clone();

    let inventory = commit(
        converge(
            &observed,
            "opencode",
            adopted_asset.clone(),
            ConvergenceAction::AdoptObserved,
        )
        .unwrap(),
    );
    assert_eq!(fs::read(target).unwrap(), reviewed_bytes);
    assert!(inventory.consumptions.iter().any(|row| {
        row.agent_id == "opencode"
            && matches!(&row.asset, AssetRef::Model { .. })
            && row.status == ConsumptionStatus::Synced
    }));
    assert!(!inventory
        .external
        .iter()
        .any(|row| row.asset == adopted_asset));
    assert!(inventory
        .external
        .iter()
        .any(|row| row.asset == other_asset));
}

#[test]
fn model_and_skill_restore_use_the_same_convergence_contract() {
    let home = TestHome::new("model-skill-convergence");
    save_profile(model(), None).unwrap();
    commit(
        MuxCore::plan(PlanOperationRequest::SetAgentConsumption(
            PlanSetAgentConsumptionRequest {
                agent_id: "codex".into(),
                selection: AgentConsumptionSelection::Model {
                    profile_ids: vec!["convergence-model".into()],
                },
            },
        ))
        .unwrap(),
    );
    let model_target = home.home.join(".codex/config.toml");
    let model_drift = fs::read_to_string(&model_target)
        .unwrap()
        .replace("desired-model", "external-model");
    fs::write(&model_target, model_drift).unwrap();
    let observed = mux_core::application::assets::list_inventory().unwrap();
    let model_inventory = commit(
        converge(
            &observed,
            "codex",
            AssetRef::Model {
                profile_id: "convergence-model".into(),
            },
            ConvergenceAction::RestoreDesired,
        )
        .unwrap(),
    );
    assert!(fs::read_to_string(model_target)
        .unwrap()
        .contains("desired-model"));
    assert!(model_inventory.consumptions.iter().any(|row| {
        row.agent_id == "codex"
            && row.status == ConsumptionStatus::Synced
            && row.asset
                == (AssetRef::Model {
                    profile_id: "convergence-model".into(),
                })
    }));

    drop(home);
    let fixture = SkillsFixture::missing_managed_link("review-changes", "agents-user");
    let observed = mux_core::application::assets::list_inventory().unwrap();
    let skill_plan = converge(
        &observed,
        "codex",
        AssetRef::Skill {
            name: "review-changes".into(),
        },
        ConvergenceAction::RestoreDesired,
    )
    .unwrap();
    commit(skill_plan);
    assert_managed_link(
        fixture.target("agents-user", "review-changes"),
        fixture.central("review-changes"),
    );
}

#[test]
fn malformed_model_input_does_not_block_an_unrelated_mcp_mutation() {
    let home = TestHome::new("isolated-observation-error");
    let model_target = home.home.join(".codex/config.toml");
    fs::create_dir_all(model_target.parent().unwrap()).unwrap();
    fs::write(
        &model_target,
        "model = \"first\"\nmodel = \"second\"\nmodel_provider = \"external\"\n",
    )
    .unwrap();
    write_manual_entry(&mcp("isolated-server")).unwrap();

    let inventory = mux_core::application::assets::list_inventory().unwrap();
    assert!(inventory.external.iter().any(|row| {
        row.agent_id == "codex"
            && matches!(
                row.status,
                ConsumptionStatus::Ambiguous | ConsumptionStatus::Unparseable
            )
    }));
    let inventory = commit(
        MuxCore::plan(PlanOperationRequest::SetAgentConsumption(
            PlanSetAgentConsumptionRequest {
                agent_id: "claude-code".into(),
                selection: AgentConsumptionSelection::Mcp {
                    asset_keys: vec!["local::stdio".into()],
                },
            },
        ))
        .unwrap(),
    );
    assert!(inventory.consumptions.iter().any(|row| {
        row.agent_id == "claude-code"
            && row.status == ConsumptionStatus::Synced
            && row.asset
                == (AssetRef::Mcp {
                    key: "local::stdio".into(),
                })
    }));
}

#[test]
fn unavailable_skill_inventory_does_not_hide_or_block_mcp() {
    let home = TestHome::new("skill-domain-isolation");
    write_manual_entry(&mcp("isolated-server")).unwrap();
    let outside = home.home.join("external-skills-root");
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, home.home.join(".mux/skills")).unwrap();

    let inventory = commit(
        MuxCore::plan(PlanOperationRequest::SetAgentConsumption(
            PlanSetAgentConsumptionRequest {
                agent_id: "claude-code".into(),
                selection: AgentConsumptionSelection::Mcp {
                    asset_keys: vec!["local::stdio".into()],
                },
            },
        ))
        .unwrap(),
    );

    assert!(inventory.consumptions.iter().any(|row| {
        row.agent_id == "claude-code"
            && row.asset
                == (AssetRef::Mcp {
                    key: "local::stdio".into(),
                })
            && row.status == ConsumptionStatus::Synced
    }));
    assert!(inventory.capability_errors.iter().any(|diagnostic| {
        diagnostic.capability == AssetCapability::Skill
            && diagnostic.code == "skill_inventory_unavailable"
    }));
    assert!(inventory.target_incidents.is_empty());
}
