//! MUX CLI — a thin clap front-end over `mux-core`. Shares `~/.mux/` (settings,
//! sources, registry) with the desktop app; all real logic lives in the core
//! crate, so the two front-ends can never drift.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, Write};

mod tui;

use clap::{Parser, Subcommand};

use mux_core::application::agents::load_agents;
use mux_core::application::assets::{
    AgentConsumptionSelection, AssetCommitRequest, AssetOperationPlan, AssetRef, CentralAssetDraft,
    McpAdoptionCandidate, PlanDeleteCentralAssetRequest, PlanEnsureAgentConsumptionRequest,
    PlanMcpAdoptionRequest, PlanSetAgentConsumptionRequest, PlanUpdateCentralAssetRequest,
};
use mux_core::application::mcp::catalog::read_registry;
use mux_core::application::mcp::operations as ops;
use mux_core::application::mcp::scanning::scan_agents;
use mux_core::application::operations::{
    CancelOperationRequest, CommitOperationRequest, OperationCommitResult, OperationPlan,
    PlanOperationRequest,
};
use mux_core::application::MuxCore;
use mux_core::domain::types::{RegistryConfig, RegistryEntry, StdioConfig};

// ---- tiny ANSI helpers (no dependency; mirrors the old picocolors output) ----
fn paint(code: &str, s: &str) -> String {
    format!("\x1b[{code}m{s}\x1b[0m")
}
fn bold(s: &str) -> String {
    paint("1", s)
}
fn dim(s: &str) -> String {
    paint("2", s)
}
fn green(s: &str) -> String {
    paint("32", s)
}
fn yellow(s: &str) -> String {
    paint("33", s)
}
fn red(s: &str) -> String {
    paint("31", s)
}

fn split_commas(raw: &str) -> Vec<String> {
    if raw.trim().is_empty() {
        Vec::new()
    } else {
        raw.split(',').map(|s| s.trim().to_string()).collect()
    }
}

fn prompt(question: &str) -> String {
    print!("{question}");
    let _ = io::stdout().flush();
    let mut line = String::new();
    io::stdin().read_line(&mut line).ok();
    line.trim().to_string()
}

#[derive(Parser)]
#[command(
    name = "mux",
    version,
    about = "MUX — central MCP, Model, and Skill assets for AI Agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Remove MUX-managed MCP relationships from enabled Agents
    Clean {
        /// Only clean a specific agent
        #[arg(long)]
        agent: Option<String>,
    },
    /// Review-safe adoption of existing global MCP observations
    Import,
    /// List all MCPs in registry
    List,
    /// Show currently active MCPs across all agents
    Status,
    /// Interactively add an MCP to registry
    Add { name: String },
    /// Remove an MCP from registry
    Remove { name: String },
    /// Export the effective MCP catalog to JSON. Prints to stdout unless --out
    /// is given.
    Export {
        /// Write to this file instead of stdout
        #[arg(long)]
        out: Option<String>,
    },
    /// Add central MCPs to Agent desired state non-interactively
    Apply {
        names: Vec<String>,
        /// Comma-separated agent names, or "all"
        #[arg(long, default_value = "all")]
        agent: String,
    },
    /// Manage AI coding agents
    Agents {
        #[command(subcommand)]
        action: Option<AgentsAction>,
    },
    /// Show the unified MCP / Model / Skill workspace
    Workspace {
        /// Print the complete revisioned snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// List central Model Profiles
    Models,
    /// List centrally managed Skills
    Skills,
    /// Upgrade mux to the latest stable release
    Upgrade,
}

#[derive(Subcommand)]
enum AgentsAction {
    /// List all agents
    List,
    /// Enable an agent
    Enable { name: String },
    /// Disable an agent
    Disable { name: String },
}

fn main() {
    let bootstrap = match mux_core::application::MuxCore::bootstrap(
        mux_core::application::bootstrap::Frontend::Cli,
    ) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("MUX recovery failed; refusing to continue: {error}");
            std::process::exit(1);
        }
    };
    for warning in bootstrap.warnings {
        eprintln!("MUX startup warning: {}", warning.message);
    }

    let cli = Cli::parse();
    match cli.command {
        Some(Command::Clean { agent }) => cmd_clean(agent.as_deref()),
        Some(Command::Import) => cmd_import(),
        Some(Command::List) => cmd_list(),
        Some(Command::Status) => cmd_status(),
        Some(Command::Add { name }) => cmd_add(&name),
        Some(Command::Remove { name }) => cmd_remove(&name),
        Some(Command::Export { out }) => cmd_export(out.as_deref()),
        Some(Command::Apply { names, agent }) => cmd_apply(names, &agent),
        Some(Command::Agents { action }) => match action {
            Some(AgentsAction::Enable { name }) => cmd_agents_set(&name, true),
            Some(AgentsAction::Disable { name }) => cmd_agents_set(&name, false),
            _ => cmd_agents_list(),
        },
        Some(Command::Workspace { json }) => cmd_workspace(json),
        Some(Command::Models) => cmd_models(),
        Some(Command::Skills) => cmd_skills(),
        Some(Command::Upgrade) => {
            cmd_upgrade();
            return; // upgrade 自身不需要再叠加被动提醒
        }
        None => {
            // No subcommand → launch the interactive TUI. Set MUX_NO_TUI to fall
            // back to printing help instead (e.g. in scripts / non-tty contexts).
            if std::env::var_os("MUX_NO_TUI").is_some() {
                let _ = <Cli as clap::CommandFactory>::command().print_help();
                println!();
            } else if let Err(e) = tui::run() {
                eprintln!("TUI 错误: {e}");
                std::process::exit(1);
            }
            return; // TUI 会话结束后不打扰
        }
    }

    // 被动更新提醒：普通命令结束后追加一行(每天最多联网检查一次，
    // MUX_NO_UPDATE_CHECK=1 关闭)。
    if let Some(notice) =
        mux_core::application::update::passive_check_notice(env!("CARGO_PKG_VERSION"))
    {
        eprintln!("\n{}", yellow(&notice));
    }
}

fn cmd_upgrade() {
    let current = env!("CARGO_PKG_VERSION");
    // 桌面 App 里带出来的 CLI（sidecar 软链）随 App 自动更新，不自行替换。
    if let Some(real) = mux_core::application::update::managed_by_desktop_app() {
        println!(
            "此 mux 由桌面 App 提供（{}），会随桌面 App 自动更新，无需单独升级。",
            real.display()
        );
        return;
    }
    println!("当前版本 v{current}，正在检查最新稳定版…");
    match mux_core::application::update::upgrade_cli(current) {
        Ok(Some(o)) => {
            println!("{}", green(&format!("✔ 已从 v{} 升级到 v{}", o.from, o.to)));
        }
        Ok(None) => println!("{}", dim("已是最新版本。")),
        Err(e) => {
            eprintln!("{}", red(&format!("升级失败: {e}")));
            std::process::exit(1);
        }
    }
}

fn cmd_list() {
    let mut entries = read_registry();
    if entries.is_empty() {
        println!(
            "{}",
            dim("No MCPs registered. Run 'mux import' to scan existing configs.")
        );
        return;
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    println!(
        "{}",
        bold(&format!("{} MCPs in registry:\n", entries.len()))
    );
    for e in &entries {
        let tags = if e.tags.is_empty() {
            String::new()
        } else {
            dim(&format!(" [{}]", e.tags.join(", ")))
        };
        println!("  {}{}", green(&e.name), tags);
        if !e.description.is_empty() {
            println!("    {}", dim(&e.description));
        }
    }
}

fn cmd_status() {
    let agents = load_agents();
    let scanned = scan_agents(&agents, None, false);
    if scanned.is_empty() {
        println!("{}", dim("No MCPs currently active in any target."));
        return;
    }
    let mut order: Vec<String> = Vec::new();
    let mut by: HashMap<String, Vec<String>> = HashMap::new();
    for s in &scanned {
        let key = format!("{} [{}]", s.agent, s.scope);
        if !by.contains_key(&key) {
            order.push(key.clone());
        }
        by.entry(key).or_default().push(s.name.clone());
    }
    for key in order {
        println!("{}", bold(&format!("  {key}:")));
        for n in &by[&key] {
            println!("    {}", green(n));
        }
        println!();
    }
}

fn cmd_import() {
    println!("{}", bold("Scanning external MCP observations...\n"));
    let candidates = match mux_core::application::assets::list_mcp_adoption_candidates() {
        Ok(candidates) => candidates,
        Err(error) => {
            eprintln!("{}", red(&format!("import failed: {error}")));
            return;
        }
    };
    let mut grouped = BTreeMap::<String, Vec<McpAdoptionCandidate>>::new();
    for candidate in candidates {
        grouped
            .entry(candidate.asset_key.clone())
            .or_default()
            .push(candidate);
    }
    let mut imported = 0;
    for (asset_key, candidates) in grouped {
        let request = PlanMcpAdoptionRequest {
            asset_key: asset_key.clone(),
            agent_ids: candidates
                .iter()
                .map(|candidate| candidate.agent_id.clone())
                .collect(),
            candidate_fingerprints: candidates
                .into_iter()
                .map(|candidate| (candidate.agent_id, candidate.fingerprint))
                .collect(),
        };
        match mux_core::application::assets::plan_mcp_adoption(request)
            .and_then(commit_reviewed_asset_plan)
        {
            Ok(_) => imported += 1,
            Err(error) => eprintln!("{}", red(&format!("  ✗ {asset_key}: {error}"))),
        }
    }
    if imported == 0 {
        println!("{}", dim("No safe MCP migration was available."));
    } else {
        println!(
            "{}",
            bold(&format!("\n{imported} central MCP asset(s) adopted."))
        );
    }
}

fn cmd_add(name: &str) {
    let key = format!("{name}::stdio");
    if read_registry().iter().any(|entry| entry.key() == key) {
        println!(
            "{}",
            yellow(&format!("\"{name}\" (stdio) already exists in registry."))
        );
        return;
    }
    let description = prompt("Description: ");
    let tags_raw = prompt("Tags (comma-separated): ");
    let command = prompt("Command (e.g. npx): ");
    let args_raw = prompt("Args (comma-separated): ");
    let args = split_commas(&args_raw);
    let entry = RegistryEntry {
        name: name.to_string(),
        description,
        tags: split_commas(&tags_raw),
        config: RegistryConfig {
            stdio: Some(StdioConfig {
                command,
                args: if args.is_empty() { None } else { Some(args) },
                env: None,
                cwd: None,
            }),
            http: None,
        },
        origin: None,
        repo: None,
    };
    let result =
        mux_core::application::assets::plan_update_central_asset(PlanUpdateCentralAssetRequest {
            draft: CentralAssetDraft::Mcp {
                existing_key: None,
                entry: Box::new(entry),
            },
        })
        .and_then(commit_reviewed_asset_plan);
    match result {
        Ok(_) => println!("{}", green(&format!("✓ {name} added to registry"))),
        Err(error) => eprintln!("{}", red(&format!("failed to add: {error}"))),
    }
}

fn cmd_remove(name: &str) {
    let entries = read_registry()
        .into_iter()
        .filter(|entry| entry.name == name)
        .collect::<Vec<_>>();
    if entries.is_empty() {
        println!("{}", red(&format!("\"{name}\" not found in registry.")));
        return;
    }
    let mut removed = 0;
    for entry in entries {
        let key = entry.key();
        let source_id = entry
            .origin
            .as_ref()
            .and_then(|origin| origin.source.clone())
            .or_else(|| entry.origin.as_ref().map(|origin| origin.kind.clone()));
        let result = mux_core::application::assets::plan_delete_central_asset(
            PlanDeleteCentralAssetRequest {
                asset: AssetRef::Mcp { key: key.clone() },
                source_id,
            },
        )
        .and_then(commit_reviewed_asset_plan);
        match result {
            Ok(_) => removed += 1,
            Err(error) => eprintln!("{}", red(&format!("  ✗ {key}: {error}"))),
        }
    }
    if removed > 0 {
        println!(
            "{}",
            green(&format!("✓ {removed} {name} asset variant(s) removed"))
        );
    }
}

fn cmd_export(out: Option<&str>) {
    let content = match ops::export_effective() {
        Ok(c) => c,
        Err(e) => {
            println!("{}", red(&format!("导出失败: {e}")));
            return;
        }
    };
    match out {
        Some(path) => match std::fs::write(path, &content) {
            Ok(_) => println!("{}", green(&format!("✓ 已导出生效配置 → {path}"))),
            Err(e) => println!("{}", red(&format!("写入 {path} 失败: {e}"))),
        },
        None => println!("{content}"),
    }
}

fn cmd_apply(names: Vec<String>, agent: &str) {
    let agents = load_agents();
    let agent_ids: Vec<String> = if agent == "all" {
        agents
            .iter()
            .filter(|(_, d)| d.enabled)
            .map(|(k, _)| k.clone())
            .collect()
    } else {
        agent.split(',').map(|s| s.trim().to_string()).collect()
    };
    let registry = read_registry();
    let mut requested = Vec::new();
    let mut errors = Vec::new();
    for name in &names {
        let keys = registry
            .iter()
            .filter(|e| &e.name == name)
            .map(RegistryEntry::key)
            .collect::<Vec<_>>();
        if keys.is_empty() {
            errors.push(format!("{name}: not in registry"));
        } else if keys.len() > 1 {
            errors.push(format!(
                "{name}: both stdio and http exist; select the asset in Desktop"
            ));
        } else {
            requested.push(keys[0].clone());
        }
    }
    let mut applied = 0;
    if errors.is_empty() {
        for agent_id in agent_ids {
            let plan = MuxCore::plan(PlanOperationRequest::EnsureAgentConsumption(
                PlanEnsureAgentConsumptionRequest {
                    agent_id: agent_id.clone(),
                    selection: AgentConsumptionSelection::Mcp {
                        asset_keys: requested.clone(),
                    },
                },
            ))
            .map_err(|error| error.to_string())
            .and_then(|plan| match plan {
                OperationPlan::Asset { plan } => Ok(*plan),
                OperationPlan::Skill { .. } => {
                    Err("Core returned a Skill plan for MCP apply".into())
                }
            });
            match plan.and_then(commit_reviewed_asset_plan) {
                Ok(_) => applied += 1,
                Err(error) => errors.push(format!("{agent_id}: {error}")),
            }
        }
    }
    for e in &errors {
        eprintln!("{}", red(&format!("  ✗ {e}")));
    }
    if applied > 0 {
        println!("{}", green(&format!("✓ Applied {applied} target(s)")));
    } else if errors.is_empty() {
        println!("{}", dim("No changes needed."));
    }
}

fn cmd_clean(agent: Option<&str>) {
    let configured = load_agents();
    let agent_ids = match agent {
        Some(agent_id) => vec![agent_id.to_string()],
        None => configured
            .iter()
            .filter(|(_, definition)| definition.enabled)
            .map(|(agent_id, _)| agent_id.clone())
            .collect(),
    };
    let inventory = match mux_core::application::assets::list_inventory() {
        Ok(inventory) => inventory,
        Err(error) => {
            eprintln!("{}", red(&format!("clean failed: {error}")));
            return;
        }
    };
    let desired_agents = inventory
        .consumptions
        .iter()
        .filter(|item| matches!(&item.asset, AssetRef::Mcp { .. }) && item.desired)
        .map(|item| item.agent_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut cleaned = 0;
    for agent_id in agent_ids {
        if !desired_agents.contains(agent_id.as_str()) {
            continue;
        }
        let result = mux_core::application::assets::plan_set_agent_consumption(
            PlanSetAgentConsumptionRequest {
                agent_id: agent_id.clone(),
                selection: AgentConsumptionSelection::Mcp {
                    asset_keys: Vec::new(),
                },
            },
        )
        .and_then(commit_reviewed_asset_plan);
        match result {
            Ok(_) => {
                cleaned += 1;
                println!("{}", green(&format!("  ✓ {agent_id} desired MCPs removed")));
            }
            Err(error) => eprintln!("{}", red(&format!("  ✗ {agent_id}: {error}"))),
        }
    }
    if cleaned == 0 {
        println!("{}", dim("No MUX-managed MCP relationship to clean."));
    }
}

fn commit_reviewed_asset_plan(
    plan: AssetOperationPlan,
) -> Result<mux_core::application::assets::ConsumptionInventory, String> {
    if !plan.can_commit {
        let _ = MuxCore::cancel(CancelOperationRequest::Asset {
            operation_id: plan.operation_id.clone(),
        });
        return Err("operation is blocked by an unresolved conflict".into());
    }
    if plan.requires_conflict_confirmation {
        let _ = MuxCore::cancel(CancelOperationRequest::Asset {
            operation_id: plan.operation_id.clone(),
        });
        return Err("operation requires Desktop review of drifted targets".into());
    }
    match MuxCore::commit(CommitOperationRequest::Asset {
        request: AssetCommitRequest {
            operation_id: plan.operation_id,
            candidate_hash: plan.candidate_hash,
            conflict_confirmation: None,
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

fn cmd_agents_list() {
    let agents = match mux_core::application::agents::list_capabilities() {
        Ok(agents) => agents,
        Err(error) => {
            eprintln!(
                "{}",
                red(&format!("failed to load Agent capabilities: {error}"))
            );
            return;
        }
    };
    println!("{}", bold("Configured Agents:\n"));
    for agent in agents {
        let status = if agent.identity.enabled {
            green("enabled")
        } else {
            dim("disabled")
        };
        let mut capabilities = Vec::new();
        if agent.capabilities.mcp.is_some() {
            capabilities.push("MCP");
        }
        if agent.capabilities.model.is_some() {
            capabilities.push("Model");
        }
        if agent.capabilities.skill.is_some() {
            capabilities.push("Skill");
        }
        println!(
            "  {} [{}] {}",
            agent.identity.name,
            status,
            dim(&capabilities.join(" · "))
        );
        if let Some(mcp) = agent.capabilities.mcp {
            if let Some(path) = mcp.config_path {
                println!("    MCP:    {}", dim(&path));
            }
        }
        if let Some(model) = agent.capabilities.model {
            if !model.config_paths.is_empty() {
                println!("    Model:  {}", dim(&model.config_paths.join(", ")));
            }
        }
        if let Some(skill) = agent.capabilities.skill {
            println!("    Skills: {}", dim(&skill.global_dir));
        }
    }
}

fn cmd_workspace(json: bool) {
    let snapshot = match mux_core::application::MuxCore::snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("{}", red(&format!("failed to load workspace: {error}")));
            return;
        }
    };
    if json {
        match serde_json::to_string_pretty(&snapshot) {
            Ok(output) => println!("{output}"),
            Err(error) => eprintln!("{}", red(&format!("failed to encode workspace: {error}"))),
        }
        return;
    }
    let central_skills = snapshot
        .assets
        .skills
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.location,
                mux_core::application::skills::SkillLocation::Central
            )
        })
        .count();
    println!("{}", bold("MUX workspace"));
    println!("  revision: {}", dim(&snapshot.revision[..12]));
    println!("  Agents:   {}", snapshot.agents.len());
    println!("  MCPs:     {}", snapshot.assets.mcp.len());
    println!("  Models:   {}", snapshot.assets.models.len());
    println!("  Skills:   {central_skills}");
    println!("  desired:  {}", snapshot.relationships.consumptions.len());
    println!("  external: {}", snapshot.relationships.external.len());
}

fn cmd_models() {
    let profiles = mux_core::application::models::list_profiles();
    if profiles.is_empty() {
        println!("{}", dim("No Model Profiles configured."));
        return;
    }
    println!("{}", bold(&format!("{} Model Profiles:\n", profiles.len())));
    for profile in profiles {
        let credential = if profile.credential_saved {
            green("credential saved")
        } else {
            dim("no credential")
        };
        println!(
            "  {} [{}] {}",
            profile.profile.name, profile.profile.provider, credential
        );
        println!("    {}", dim(&profile.profile.model));
    }
}

fn cmd_skills() {
    let inventory = match mux_core::application::skills::list_inventory() {
        Ok(inventory) => inventory,
        Err(error) => {
            let parts = error.into_command_parts();
            eprintln!(
                "{}",
                red(&format!("failed to load Skills: {}", parts.message))
            );
            return;
        }
    };
    let mut names = inventory
        .items
        .iter()
        .filter_map(|item| {
            matches!(
                item.location,
                mux_core::application::skills::SkillLocation::Central
            )
            .then_some(item.name.as_str())
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    if names.is_empty() {
        println!("{}", dim("No centrally managed Skills."));
        return;
    }
    println!("{}", bold(&format!("{} managed Skills:\n", names.len())));
    for name in names {
        println!("  {}", green(name));
    }
}

fn cmd_agents_set(name: &str, enabled: bool) {
    match mux_core::application::agents::set_enabled(name, enabled) {
        Ok(()) => {
            let verb = if enabled { "enabled" } else { "disabled" };
            println!("{}", green(&format!("✓ {name} {verb}")));
        }
        Err(e) => eprintln!("{}", red(&format!("failed to save agents: {e}"))),
    }
}
