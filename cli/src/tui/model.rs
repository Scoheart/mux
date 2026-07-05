//! The whole application state. Owned by the event loop; mutated only by
//! `update`; read only by `view`.

use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Registry,
    Sources,
    Agents,
}

impl Screen {
    pub fn title(self) -> &'static str {
        match self {
            Screen::Registry => "Registry",
            Screen::Sources => "来源",
            Screen::Agents => "Agents",
        }
    }

    pub const ALL: [Screen; 3] = [Screen::Registry, Screen::Sources, Screen::Agents];

    pub fn next(self) -> Screen {
        match self {
            Screen::Registry => Screen::Sources,
            Screen::Sources => Screen::Agents,
            Screen::Agents => Screen::Registry,
        }
    }

    pub fn prev(self) -> Screen {
        match self {
            Screen::Registry => Screen::Agents,
            Screen::Sources => Screen::Registry,
            Screen::Agents => Screen::Sources,
        }
    }
}

/// Cache-size snapshot from the initial load — Phase 1 placeholder for the real
/// data caches that Phase 2 fills in.
pub struct Counts {
    pub registry: usize,
    pub agents: usize,
    pub sources: usize,
    pub installed: usize,
}

pub struct Model {
    pub screen: Screen,
    pub should_quit: bool,
    pub tick: u64,
    pub loading: bool,
    pub counts: Option<Counts>,
    pub status: Option<String>,
}

impl Model {
    pub fn new() -> Self {
        Self {
            screen: Screen::Registry,
            should_quit: false,
            tick: 0,
            loading: true,
            counts: None,
            status: None,
        }
    }

    /// How long the loop blocks waiting for the next message. Long while nothing
    /// animates (idle TUI = zero CPU); effect results and input arrive
    /// immediately regardless via the channel.
    pub fn tick_interval(&self) -> Duration {
        Duration::from_secs(3600)
    }
}
