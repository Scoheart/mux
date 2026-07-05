//! Every event that can change the model: an input key (already low-level from
//! crossterm), a tick, or the result of an async effect. `update` is the only
//! consumer.

use crossterm::event::KeyEvent;

use mux_core::agents::AgentInfo;
use mux_core::ops::InstalledMcp;
use mux_core::sources::SourceView;
use mux_core::types::RegistryEntry;

pub enum Msg {
    /// First message: kick off the initial data load.
    Init,
    /// A key press from the input thread.
    Key(KeyEvent),
    /// Terminal resized — force a redraw (no state change).
    Redraw,
    /// Animation/idle tick (unused while nothing animates).
    Tick,
    /// Result of `Effect::LoadAll`: every cache, read from core.
    Loaded(Box<LoadedData>),
    /// Result of a mutation effect (install/enable/disable/delete): a human
    /// label plus success/error. Ok triggers a reload.
    Mutated {
        label: String,
        result: Result<(), String>,
    },
}

/// The four caches plus the set of user-overridden keys, loaded together.
pub struct LoadedData {
    pub registry: Vec<RegistryEntry>,
    pub custom_keys: Vec<String>,
    pub sources: Vec<SourceView>,
    pub agents: Vec<AgentInfo>,
    pub installed: Vec<InstalledMcp>,
}
