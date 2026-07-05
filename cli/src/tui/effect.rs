//! The impure edge. `update` returns `Effect`s describing I/O; the runner
//! executes each on its own thread (so a slow network fetch never blocks input
//! or other effects) and posts a result `Msg` back onto the loop's channel.

use std::sync::mpsc::Sender;
use std::thread;

use mux_core::agents::list_infos;
use mux_core::ops::scan_installed;
use mux_core::registry::{read_registry, user_override_keys};
use mux_core::sources::list_views;

use super::message::{LoadedData, Msg};

pub enum Effect {
    /// Read all caches from core.
    LoadAll,
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

fn run_effect(eff: Effect) -> Msg {
    match eff {
        Effect::LoadAll => Msg::Loaded(Box::new(LoadedData {
            registry: read_registry(),
            custom_keys: user_override_keys(),
            sources: list_views(),
            agents: list_infos(),
            installed: scan_installed(None),
        })),
    }
}
