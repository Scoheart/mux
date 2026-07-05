//! The impure edge. `update` returns `Effect`s describing I/O; the runner
//! executes each on its own thread (so a slow network fetch never blocks input
//! or other effects) and posts a result `Msg` back onto the loop's channel.

use std::sync::mpsc::Sender;
use std::thread;

use mux_core::agents::list_infos;
use mux_core::ops::scan_installed;
use mux_core::registry::read_registry;
use mux_core::sources::list_views;

use super::message::Msg;

pub enum Effect {
    /// Read all four caches from core (Phase 1: just their sizes).
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
        Effect::LoadAll => Msg::Loaded {
            registry: read_registry().len(),
            agents: list_infos().len(),
            sources: list_views().len(),
            installed: scan_installed(None).len(),
        },
    }
}
