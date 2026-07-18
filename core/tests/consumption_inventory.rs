#![cfg(unix)]

mod support;

use mux_core::consumption::{
    list_consumption_inventory, plan_set_agent_consumption, AgentConsumptionSelection, AssetRef,
    ConsumptionStatus, McpConsumptionRecord, PlanSetAgentConsumptionRequest,
};
use mux_core::ops::install;
use mux_core::r#override::OverridePatch;
use mux_core::registry::write_manual_entry;
use mux_core::settings::mutate_settings;
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
                protocol: ModelProtocol::AnthropicMessages,
                base_url: "https://example.invalid".into(),
                model: "example".into(),
                context_window: None,
                max_output_tokens: None,
                reasoning: false,
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

#[test]
fn unassigned_model_configuration_is_external_and_blocks_takeover() {
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
            ModelProfile {
                id: "inventory-profile".into(),
                name: "Inventory".into(),
                protocol: ModelProtocol::OpenaiResponses,
                base_url: "https://example.invalid".into(),
                model: "example".into(),
                context_window: None,
                max_output_tokens: None,
                reasoning: false,
            },
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
    assert!(!plan.can_commit);
    assert!(plan
        .warnings
        .iter()
        .any(|warning| warning.contains("model_external_unmanaged")));
    assert_eq!(
        fs::read_to_string(target).unwrap(),
        "model = \"external-model\"\nmodel_provider = \"external-provider\"\n"
    );
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
