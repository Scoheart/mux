//! The impure edge. `update` returns `Effect`s describing I/O; the runner
//! executes each on its own thread (so a slow network fetch never blocks input
//! or other effects) and posts a result `Msg` back onto the loop's channel.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::Sender;
use std::thread;

use mux_core::application::agents::list_infos;
use mux_core::application::assets::{
    AssetCommitRequest, AssetOperationPlan, AssetRef, CentralAssetDraft, ConsumptionInventory,
    PlanDeleteCentralAssetRequest, PlanMcpAdoptionRequest, PlanReapplyMcpRequest,
    PlanSetMcpEnabledRequest, PlanUpdateAssetConsumersRequest, PlanUpdateCentralAssetRequest,
};
use mux_core::application::mcp::catalog::{read_registry, read_registry_all, user_override_keys};
use mux_core::application::mcp::operations::{parse_pasted_entries, scan_installed, ResyncOutcome};
use mux_core::application::mcp::sources;
use mux_core::application::operations::{
    CancelOperationRequest, CommitOperationRequest, OperationCommitResult, OperationPlan,
    PlanOperationRequest,
};
use mux_core::application::MuxCore;
use mux_core::domain::types::{AgentDefinition, RegistryEntry};

use super::message::{LoadedData, Msg};

/// A side effect to run off the UI thread. Mutations carry owned params so a
/// pending one can be parked in a Confirm modal until the user commits.
pub enum Effect {
    /// Read all caches from core.
    LoadAll,
    /// Install a catalog entry into the given agents (global scope).
    Install {
        server: String,
        transport: String,
        agents: Vec<String>,
    },
    /// Re-enable a previously disabled server for one agent.
    Enable {
        server: String,
        transport: String,
        agent: String,
    },
    /// Disable (snapshot + remove) a server for one agent.
    Disable {
        server: String,
        transport: String,
        agent: String,
    },
    /// Hard-delete a server from one agent.
    Delete {
        server: String,
        transport: String,
        agent: String,
    },
    /// Save a catalog entry (create/edit). Existing entries retain their original
    /// durable identity; renames are represented as an explicit create elsewhere.
    UpsertEntry {
        entry: RegistryEntry,
        existing_key: Option<String>,
    },
    /// Revert a custom entry to its source-provided default (or remove it).
    RevertEntry { name: String, transport: String },
    /// Import MCP servers from a pasted JSON/TOML blob.
    ImportPaste(String),
    /// Subscribe to a remote source URL (network).
    Subscribe { url: String, name: Option<String> },
    /// Import a local file as a source.
    AddLocal { path: String, name: Option<String> },
    /// Re-fetch/re-read a source (network for remote).
    RefreshSource { id: String },
    /// Toggle a source enabled/disabled.
    SetSourceEnabled { id: String, on: bool },
    /// Remove a source and its cache.
    RemoveSource { id: String },
    /// Re-scan agents and register newly discovered servers.
    ImportDiscovered,
    /// Create or edit an agent definition.
    PutAgent {
        id: String,
        def: AgentDefinition,
        overwrite: bool,
    },
    /// Toggle any Agent definition without reconstructing or dropping its
    /// MCP/Model/Skill capability metadata.
    SetAgentEnabled { id: String, enabled: bool },
    /// Re-stamp an entry's current config into the agents that have it installed
    /// (global). force=false skips customized installs; force=true overwrites.
    ResyncEntry {
        name: String,
        transport: String,
        force: bool,
    },
    /// Delete a manual/discovered catalog entry and uninstall it from all agents.
    ForgetEntry { name: String, transport: String },
}

pub struct EffectRunner {
    tx: Sender<Msg>,
}

impl EffectRunner {
    pub fn new(tx: Sender<Msg>) -> Self {
        Self { tx }
    }

    /// Run one effect off the UI thread; its result `Msg` lands back on the loop.
    pub fn spawn(&self, eff: Effect) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let msg = run_effect(eff);
            let _ = tx.send(msg);
        });
    }
}

/// Join per-agent errors into one line for the status bar.
fn commit_plan(
    plan: AssetOperationPlan,
    confirm_conflict: bool,
) -> Result<ConsumptionInventory, String> {
    if !plan.can_commit {
        let _ = MuxCore::cancel(CancelOperationRequest::Asset {
            operation_id: plan.operation_id.clone(),
        });
        return Err(plan.warnings.join("；"));
    }
    if plan.requires_conflict_confirmation && !confirm_conflict {
        let _ = MuxCore::cancel(CancelOperationRequest::Asset {
            operation_id: plan.operation_id.clone(),
        });
        return Err("该操作会覆盖已漂移的 Agent 配置，需要显式确认".into());
    }
    match MuxCore::commit(CommitOperationRequest::Asset {
        request: AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash.clone(),
            conflict_confirmation: plan
                .requires_conflict_confirmation
                .then_some(plan.candidate_hash),
        },
    })
    .map_err(|error| error.to_string())?
    {
        OperationCommitResult::Asset { inventory } => Ok(inventory),
        OperationCommitResult::Skill { .. } => {
            Err("Core returned a Skill result for an asset commit".into())
        }
    }
}

fn update_mcp_consumers(
    asset_key: String,
    add_agent_ids: Vec<String>,
    remove_agent_ids: Vec<String>,
) -> Result<(), String> {
    let plan = MuxCore::plan(PlanOperationRequest::UpdateAssetConsumers(
        PlanUpdateAssetConsumersRequest {
            asset: AssetRef::Mcp { key: asset_key },
            add_agent_ids,
            remove_agent_ids,
        },
    ))
    .map_err(|error| error.to_string())?;
    let OperationPlan::Asset { plan } = plan else {
        return Err("Core returned a Skill plan for an MCP consumer change".into());
    };
    commit_plan(*plan, false).map(|_| ())
}

fn upsert_entry(
    entry: RegistryEntry,
    existing_key: Option<String>,
    confirm_conflict: bool,
) -> Result<Vec<String>, String> {
    let plan =
        mux_core::application::assets::plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Mcp {
                existing_key,
                entry: Box::new(entry),
            },
        })?;
    let affected = plan.affected_agent_ids.clone();
    commit_plan(plan, confirm_conflict)?;
    Ok(affected)
}

fn put_agent_preserving_skills(
    id: String,
    definition: AgentDefinition,
    overwrite: bool,
) -> Result<(), String> {
    // Core owns omission semantics and performs a compare-and-swap against the
    // current definition. Preloading Skills here can turn a stale frontend read
    // into an explicit overwrite of a newer capability.
    mux_core::application::agents::put(id, definition, overwrite)
}

fn delete_entry_source(name: &str, transport: &str, source_id: &str) -> Result<(), String> {
    let plan =
        mux_core::application::assets::plan_delete_central_asset(PlanDeleteCentralAssetRequest {
            asset: AssetRef::Mcp {
                key: format!("{name}::{transport}"),
            },
            source_id: Some(source_id.to_string()),
        })?;
    commit_plan(plan, false).map(|_| ())
}

fn import_discovered() -> Result<usize, String> {
    let candidates = mux_core::application::assets::list_mcp_adoption_candidates()?;
    let mut grouped = BTreeMap::new();
    for candidate in candidates {
        grouped
            .entry(candidate.asset_key.clone())
            .or_insert_with(Vec::new)
            .push(candidate);
    }
    let mut imported = 0;
    for (asset_key, candidates) in grouped {
        let plan = mux_core::application::assets::plan_mcp_adoption(PlanMcpAdoptionRequest {
            asset_key,
            agent_ids: candidates
                .iter()
                .map(|candidate| candidate.agent_id.clone())
                .collect(),
            candidate_fingerprints: candidates
                .into_iter()
                .map(|candidate| (candidate.agent_id, candidate.fingerprint))
                .collect(),
        })?;
        commit_plan(plan, false)?;
        imported += 1;
    }
    Ok(imported)
}

fn forget_entry(name: &str, transport: &str) -> Result<(), String> {
    let key = format!("{name}::{transport}");
    let source_ids = read_registry_all()
        .into_iter()
        .filter(|item| item.entry.key() == key)
        .filter_map(|item| {
            let origin = item.entry.origin?;
            origin.source.or(Some(origin.kind))
        })
        .filter(|source| matches!(source.as_str(), "manual" | "discovered"))
        .collect::<BTreeSet<_>>();
    if source_ids.is_empty() {
        return Err("该条目不属于可删除的手动或探索来源".into());
    }
    for source_id in source_ids.into_iter().rev() {
        delete_entry_source(name, transport, &source_id)?;
    }
    Ok(())
}

fn resync_entry(name: &str, transport: &str, force: bool) -> Result<ResyncOutcome, String> {
    let plan = mux_core::application::assets::plan_reapply_mcp(PlanReapplyMcpRequest {
        asset_key: format!("{name}::{transport}"),
    })?;
    if plan.requires_conflict_confirmation && !force {
        let skipped_customized = plan.affected_agent_ids.clone();
        let _ = mux_core::application::assets::cancel_asset_operation(&plan.operation_id);
        return Ok(ResyncOutcome {
            synced: Vec::new(),
            skipped_customized,
        });
    }
    let synced = plan.affected_agent_ids.clone();
    commit_plan(plan, force)?;
    Ok(ResyncOutcome {
        synced,
        skipped_customized: Vec::new(),
    })
}

fn run_effect(eff: Effect) -> Msg {
    match eff {
        Effect::LoadAll => Msg::Loaded(Box::new(LoadedData {
            registry: read_registry(),
            custom_keys: user_override_keys(),
            sources: sources::list_views(),
            agents: list_infos(),
            installed: scan_installed(None),
        })),
        Effect::Install {
            server,
            transport,
            agents,
        } => Msg::Mutated {
            label: format!("安装 {server}"),
            result: update_mcp_consumers(format!("{server}::{transport}"), agents, Vec::new()),
        },
        Effect::Enable {
            server,
            transport,
            agent,
        } => Msg::Mutated {
            label: format!("启用 {server}"),
            result: mux_core::application::assets::plan_set_mcp_enabled(PlanSetMcpEnabledRequest {
                agent_id: agent,
                asset_key: format!("{server}::{transport}"),
                enabled: true,
            })
            .and_then(|plan| commit_plan(plan, false).map(|_| ())),
        },
        Effect::Disable {
            server,
            transport,
            agent,
        } => Msg::Mutated {
            label: format!("停用 {server}"),
            result: mux_core::application::assets::plan_set_mcp_enabled(PlanSetMcpEnabledRequest {
                agent_id: agent,
                asset_key: format!("{server}::{transport}"),
                enabled: false,
            })
            .and_then(|plan| commit_plan(plan, false).map(|_| ())),
        },
        Effect::Delete {
            server,
            transport,
            agent,
        } => Msg::Mutated {
            label: format!("删除 {server}"),
            result: update_mcp_consumers(format!("{server}::{transport}"), Vec::new(), vec![agent]),
        },
        Effect::UpsertEntry {
            entry,
            existing_key,
        } => {
            let name = entry.name.clone();
            let result = upsert_entry(entry, existing_key, false);
            // Saving auto-syncs the new config to installed agents — say so.
            let label = match &result {
                Ok(synced) if !synced.is_empty() => {
                    format!("保存 {}（已同步 {} 个 agent）", name, synced.len())
                }
                _ => format!("保存 {name}"),
            };
            Msg::Mutated {
                label,
                result: result.map(|_| ()),
            }
        }
        Effect::RevertEntry { name, transport } => Msg::Mutated {
            label: format!("恢复默认 {name}"),
            result: delete_entry_source(&name, &transport, "manual"),
        },
        Effect::ImportPaste(text) => {
            let result = parse_pasted_entries(&text).and_then(|entries| {
                let count = entries.len();
                for entry in entries {
                    let existing_key = read_registry()
                        .iter()
                        .any(|candidate| candidate.key() == entry.key())
                        .then(|| entry.key());
                    upsert_entry(entry, existing_key, false)?;
                }
                Ok(count)
            });
            match result {
                Ok(count) => Msg::Mutated {
                    label: format!("导入 {count} 个 server"),
                    result: Ok(()),
                },
                Err(error) => Msg::Mutated {
                    label: "导入".into(),
                    result: Err(error),
                },
            }
        }
        Effect::Subscribe { url, name } => Msg::Mutated {
            label: "订阅来源".into(),
            result: sources::subscribe(url, name).map(|_| ()),
        },
        Effect::AddLocal { path, name } => Msg::Mutated {
            label: "导入本地来源".into(),
            result: sources::add_local(path, name).map(|_| ()),
        },
        Effect::RefreshSource { id } => Msg::Mutated {
            label: "刷新来源".into(),
            result: sources::refresh(id).map(|_| ()),
        },
        Effect::SetSourceEnabled { id, on } => Msg::Mutated {
            label: if on { "启用来源" } else { "停用来源" }.into(),
            result: sources::set_enabled(id, on),
        },
        Effect::RemoveSource { id } => Msg::Mutated {
            label: "删除来源".into(),
            result: sources::remove(id),
        },
        Effect::ImportDiscovered => match import_discovered() {
            Ok(n) => Msg::Mutated {
                label: format!("探索到 {n} 个新 server"),
                result: Ok(()),
            },
            Err(e) => Msg::Mutated {
                label: "探索".into(),
                result: Err(e),
            },
        },
        Effect::PutAgent { id, def, overwrite } => Msg::Mutated {
            label: format!("保存 agent {id}"),
            result: put_agent_preserving_skills(id, def, overwrite),
        },
        Effect::SetAgentEnabled { id, enabled } => Msg::Mutated {
            label: format!("{} agent {id}", if enabled { "启用" } else { "停用" }),
            result: mux_core::application::agents::set_enabled(&id, enabled),
        },
        Effect::ResyncEntry {
            name,
            transport,
            force,
        } => {
            let result = resync_entry(&name, &transport, force);
            Msg::Resynced {
                name,
                transport,
                result,
            }
        }
        Effect::ForgetEntry { name, transport } => Msg::Mutated {
            label: format!("删除 {name}"),
            result: forget_entry(&name, &transport),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mux_core::domain::types::{AgentInstallProbe, AgentSkillsCapability};

    #[test]
    fn mcp_form_edit_preserves_custom_agent_skills() {
        let _home = mux_core::testenv::TestHome::new("tui-agent-skills");
        let skills = AgentSkillsCapability {
            target_id: "custom-user".into(),
            global_dir: "~/.custom/skills".into(),
            aliases: Vec::new(),
            docs: "https://example.invalid/skills".into(),
            evidence: "official-source".into(),
            verified_at: "2026-07-23".into(),
            probes: vec![AgentInstallProbe::Command {
                name: "custom".into(),
            }],
        };
        mux_core::application::agents::put(
            "custom".into(),
            AgentDefinition {
                global: Some("~/.custom/mcp.json".into()),
                format: "json".into(),
                key: "mcpServers".into(),
                enabled: true,
                skills: Some(skills.clone()),
                ..AgentDefinition::default()
            },
            false,
        )
        .unwrap();

        put_agent_preserving_skills(
            "custom".into(),
            AgentDefinition {
                global: Some("~/.custom/updated.json".into()),
                format: "json".into(),
                key: "mcpServers".into(),
                enabled: true,
                ..AgentDefinition::default()
            },
            true,
        )
        .unwrap();

        let loaded = mux_core::application::agents::load_agents();
        assert_eq!(loaded["custom"].skills.as_ref(), Some(&skills));
        assert_eq!(
            loaded["custom"].global.as_deref(),
            Some("~/.custom/updated.json")
        );
    }

    #[test]
    fn set_enabled_effect_preserves_skill_only_definition() {
        let _home = mux_core::testenv::TestHome::new("tui-agent-toggle");
        let skills = AgentSkillsCapability {
            target_id: "skill-user".into(),
            global_dir: "~/.skill-only/skills".into(),
            aliases: Vec::new(),
            docs: "https://example.invalid/skills".into(),
            evidence: "official-source".into(),
            verified_at: "2026-07-23".into(),
            probes: vec![AgentInstallProbe::Command {
                name: "skill-only".into(),
            }],
        };
        mux_core::application::agents::put(
            "skill-only".into(),
            AgentDefinition {
                enabled: true,
                skills: Some(skills.clone()),
                ..AgentDefinition::default()
            },
            false,
        )
        .unwrap();

        let message = run_effect(Effect::SetAgentEnabled {
            id: "skill-only".into(),
            enabled: false,
        });
        assert!(matches!(message, Msg::Mutated { result: Ok(()), .. }));
        let loaded = mux_core::application::agents::load_agents();
        assert!(!loaded["skill-only"].enabled);
        assert_eq!(loaded["skill-only"].skills.as_ref(), Some(&skills));
    }
}
