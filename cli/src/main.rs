//! MUX CLI — a thin clap front-end over `mux-core`. Shares `~/.mux/` (settings,
//! sources, registry) with the desktop app; all real logic lives in the core
//! crate, so the two front-ends can never drift.

use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

mod tui;

use clap::{Parser, Subcommand};

use mux_core::agents::{load_agents, save_agents};
use mux_core::ops;
use mux_core::registry::{
    delete_registry_entry, migrate_registry_to_sources, read_registry, write_manual_entry,
};
use mux_core::scanner::scan_agents;
use mux_core::settings::migrate_if_needed;
use mux_core::types::{RegistryConfig, RegistryEntry, RegistryOrigin, StdioConfig};

// ---- tiny ANSI helpers (no dependency; mirrors the old picocolors output) ----
fn paint(code: &str, s: &str) -> String {
    format!("\x1b[{}m{}\x1b[0m", code, s)
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
    print!("{}", question);
    let _ = io::stdout().flush();
    let mut line = String::new();
    io::stdin().read_line(&mut line).ok();
    line.trim().to_string()
}

#[derive(Parser)]
#[command(
    name = "mux",
    version,
    about = "MCP Multiplexer — Unified MCP Server configuration manager"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Clear all MCP configurations from enabled agents
    Clean {
        /// Only clean a specific agent
        #[arg(long)]
        agent: Option<String>,
    },
    /// Scan existing configs and import MCPs to registry
    Import,
    /// List all MCPs in registry
    List,
    /// Show currently active MCPs across all agents
    Status,
    /// Interactively add an MCP to registry
    Add { name: String },
    /// Remove an MCP from registry
    Remove { name: String },
    /// Export all manually-added MCPs to a config file (JSON). Prints to stdout
    /// unless --out is given.
    Export {
        /// Write to this file instead of stdout
        #[arg(long)]
        out: Option<String>,
    },
    /// Apply MCPs non-interactively
    Apply {
        names: Vec<String>,
        /// Scope: global, project, both
        #[arg(long, default_value = "global")]
        scope: String,
        /// Comma-separated agent names, or "all"
        #[arg(long, default_value = "all")]
        agent: String,
        /// Project directory for project scope
        #[arg(long)]
        project: Option<String>,
    },
    /// Manage AI coding agents
    Agents {
        #[command(subcommand)]
        action: Option<AgentsAction>,
    },
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
    // Fold any legacy ~/.mux files into a single settings.json on first run,
    // then move manual/discovered registry entries into managed source files so
    // the CLI and desktop share the same source-based storage.
    migrate_if_needed();
    migrate_registry_to_sources();

    let cli = Cli::parse();
    match cli.command {
        Some(Command::Clean { agent }) => cmd_clean(agent.as_deref()),
        Some(Command::Import) => cmd_import(),
        Some(Command::List) => cmd_list(),
        Some(Command::Status) => cmd_status(),
        Some(Command::Add { name }) => cmd_add(&name),
        Some(Command::Remove { name }) => cmd_remove(&name),
        Some(Command::Export { out }) => cmd_export(out.as_deref()),
        Some(Command::Apply {
            names,
            scope,
            agent,
            project,
        }) => cmd_apply(names, &scope, &agent, project.as_deref()),
        Some(Command::Agents { action }) => match action {
            Some(AgentsAction::Enable { name }) => cmd_agents_set(&name, true),
            Some(AgentsAction::Disable { name }) => cmd_agents_set(&name, false),
            _ => cmd_agents_list(),
        },
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
                eprintln!("TUI 错误: {}", e);
                std::process::exit(1);
            }
            return; // TUI 会话结束后不打扰
        }
    }

    // 被动更新提醒：普通命令结束后追加一行(每天最多联网检查一次，
    // MUX_NO_UPDATE_CHECK=1 关闭)。
    if let Some(notice) = mux_core::update::passive_check_notice(env!("CARGO_PKG_VERSION")) {
        eprintln!("\n{}", yellow(&notice));
    }
}

fn cmd_upgrade() {
    let current = env!("CARGO_PKG_VERSION");
    println!("当前版本 v{}，正在检查最新稳定版…", current);
    match mux_core::update::upgrade_cli(current) {
        Ok(Some(o)) => {
            println!(
                "{}",
                green(&format!("✔ 已从 v{} 升级到 v{}", o.from, o.to))
            );
        }
        Ok(None) => println!("{}", dim("已是最新版本。")),
        Err(e) => {
            eprintln!("{}", red(&format!("升级失败: {}", e)));
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
    println!("{}", bold(&format!("{} MCPs in registry:\n", entries.len())));
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
        println!("{}", bold(&format!("  {}:", key)));
        for n in &by[&key] {
            println!("    {}", green(n));
        }
        println!();
    }
}

fn cmd_import() {
    println!("{}", bold("Scanning targets...\n"));
    match ops::import_discovered(None) {
        Ok(n) => println!("{}", bold(&format!("\n{} new MCPs imported.", n))),
        Err(e) => eprintln!("{}", red(&format!("import failed: {}", e))),
    }
}

fn cmd_add(name: &str) {
    // `add` only creates stdio (command-based) entries → key is `${name}::stdio`.
    let existing: HashSet<String> = read_registry().iter().map(|e| e.key()).collect();
    if existing.contains(&format!("{}::stdio", name)) {
        println!(
            "{}",
            yellow(&format!("\"{}\" (stdio) already exists in registry.", name))
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
            }),
            http: None,
        },
        origin: Some(RegistryOrigin {
            kind: "manual".into(),
            agent: None,
            scope: None,
            source: None,
        }),
        repo: None,
    };
    match write_manual_entry(&entry) {
        Ok(()) => println!("{}", green(&format!("✓ {} added to registry", name))),
        Err(e) => eprintln!("{}", red(&format!("failed to add: {}", e))),
    }
}

fn cmd_remove(name: &str) {
    // A name may have a stdio and/or an http variant; clear whichever exist.
    let mut removed = false;
    for t in ["stdio", "http"] {
        if delete_registry_entry(name, t).is_ok() {
            removed = true;
        }
    }
    if removed {
        println!("{}", green(&format!("✓ {} removed from registry", name)));
    } else {
        println!("{}", red(&format!("\"{}\" not found in registry.", name)));
    }
}

fn cmd_export(out: Option<&str>) {
    let content = match ops::export_manual() {
        Ok(c) => c,
        Err(e) => {
            println!("{}", red(&format!("导出失败: {}", e)));
            return;
        }
    };
    match out {
        Some(path) => match std::fs::write(path, &content) {
            Ok(_) => println!("{}", green(&format!("✓ 已导出手动添加的 MCP → {}", path))),
            Err(e) => println!("{}", red(&format!("写入 {} 失败: {}", path, e))),
        },
        None => println!("{}", content),
    }
}

fn cmd_apply(names: Vec<String>, scope: &str, agent: &str, project: Option<&str>) {
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
    let scopes: Vec<&str> = if scope == "both" {
        vec!["global", "project"]
    } else {
        vec![scope]
    };
    let overrides = HashMap::new();
    let mut applied = 0;
    let mut errors: Vec<String> = Vec::new();
    for name in &names {
        let mut transports: Vec<&'static str> = registry
            .iter()
            .filter(|e| &e.name == name)
            .map(|e| e.transport())
            .collect();
        transports.sort_unstable();
        transports.dedup();
        if transports.is_empty() {
            errors.push(format!("{}: not in registry", name));
            continue;
        }
        for t in transports {
            for sc in &scopes {
                match ops::install(name, t, sc, &agent_ids, project, &overrides) {
                    Ok(()) => applied += 1,
                    Err(e) => errors.extend(e),
                }
            }
        }
    }
    for e in &errors {
        eprintln!("{}", red(&format!("  ✗ {}", e)));
    }
    if applied > 0 {
        println!("{}", green(&format!("✓ Applied {} target(s)", applied)));
    } else if errors.is_empty() {
        println!("{}", dim("No changes needed."));
    }
}

fn cmd_clean(agent: Option<&str>) {
    let cleaned = ops::clean(agent);
    if cleaned.is_empty() {
        println!("{}", dim("Nothing to clean."));
        return;
    }
    for name in &cleaned {
        println!("{}", green(&format!("  ✓ {} [global] cleaned", name)));
    }
    println!(
        "{}",
        bold(&format!("\n{} agent(s) cleaned.", cleaned.len()))
    );
}

fn cmd_agents_list() {
    let config = load_agents();
    println!("{}", bold("Configured agents:\n"));
    for (name, def) in &config {
        let status = if def.enabled {
            green("enabled")
        } else {
            dim("disabled")
        };
        println!("  {} [{}]", name, status);
        if let Some(g) = &def.global {
            println!("    global:  {}", dim(g));
        }
        if let Some(p) = &def.project {
            println!("    project: {}", dim(p));
        }
    }
}

fn cmd_agents_set(name: &str, enabled: bool) {
    let mut config = load_agents();
    let Some(def) = config.get_mut(name) else {
        println!("{}", red(&format!("Agent \"{}\" not found.", name)));
        return;
    };
    def.enabled = enabled;
    match save_agents(&config) {
        Ok(()) => {
            let verb = if enabled { "enabled" } else { "disabled" };
            println!("{}", green(&format!("✓ {} {}", name, verb)));
        }
        Err(e) => eprintln!("{}", red(&format!("failed to save agents: {}", e))),
    }
}
