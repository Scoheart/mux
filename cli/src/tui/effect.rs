//! The impure edge. `update` returns `Effect`s describing I/O; the runner
//! executes each on its own thread (so a slow network fetch never blocks input
//! or other effects) and posts a result `Msg` back onto the loop's channel.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::thread;

use mux_core::agents::list_infos;
use mux_core::ops::{self, scan_installed};
use mux_core::registry::{read_registry, user_override_keys};
use mux_core::sources::list_views;
use mux_core::types::RegistryEntry;

use super::message::{LoadedData, Msg};

/// A side effect to run off the UI thread. Mutations carry owned params so a
/// pending one can be parked in a Confirm modal until the user commits.
pub enum Effect {
    /// Read all caches from core.
    LoadAll,
    /// Install a catalog entry into the given agents (global scope).
    Install { server: String, transport: String, agents: Vec<String> },
    /// Re-enable a previously disabled server for one agent.
    Enable { server: String, transport: String, agent: String },
    /// Disable (snapshot + remove) a server for one agent.
    Disable { server: String, transport: String, agent: String },
    /// Hard-delete a server from one agent.
    Delete { server: String, transport: String, agent: String },
    /// Save a catalog entry (create/edit). `delete_old` handles a rename: the old
    /// key is removed after the upsert, sequentially, so the shared manual source
    /// file isn't clobbered by a concurrent write.
    UpsertEntry { entry: RegistryEntry, delete_old: Option<(String, String)> },
    /// Revert a custom entry to its source-provided default (or remove it).
    RevertEntry { name: String, transport: String },
    /// Import MCP servers from a pasted JSON/TOML blob.
    ImportPaste(String),
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
fn join(r: Result<(), Vec<String>>) -> Result<(), String> {
    r.map_err(|v| v.join("；"))
}

fn run_effect(eff: Effect) -> Msg {
    match eff {
        Effect::LoadAll => Msg::Loaded(Box::new(LoadedData {
            registry: read_registry(),
            custom_keys: user_override_keys(),
            sources: list_views(),
            agents: list_infos(),
            installed: scan_installed(None),
        })),
        Effect::Install { server, transport, agents } => Msg::Mutated {
            label: format!("安装 {}", server),
            result: join(ops::install(&server, &transport, "global", &agents, None, &HashMap::new())),
        },
        Effect::Enable { server, transport, agent } => Msg::Mutated {
            label: format!("启用 {}", server),
            result: join(ops::enable(&server, &transport, "global", &[agent], None)),
        },
        Effect::Disable { server, transport, agent } => Msg::Mutated {
            label: format!("停用 {}", server),
            result: join(ops::disable(&server, &transport, "global", &[agent], None)),
        },
        Effect::Delete { server, transport, agent } => Msg::Mutated {
            label: format!("删除 {}", server),
            result: join(ops::delete(&server, &transport, "global", &[agent], None)),
        },
        Effect::UpsertEntry { entry, delete_old } => {
            let name = entry.name.clone();
            let result = ops::upsert_entry(entry).and_then(|()| match delete_old {
                Some((n, t)) => ops::remove_entry(&n, &t),
                None => Ok(()),
            });
            Msg::Mutated { label: format!("保存 {}", name), result }
        }
        Effect::RevertEntry { name, transport } => Msg::Mutated {
            label: format!("恢复默认 {}", name),
            result: ops::remove_entry(&name, &transport),
        },
        Effect::ImportPaste(text) => match ops::import_pasted(&text) {
            Ok(names) => Msg::Mutated { label: format!("导入 {} 个 server", names.len()), result: Ok(()) },
            Err(e) => Msg::Mutated { label: "导入".into(), result: Err(e) },
        },
    }
}
