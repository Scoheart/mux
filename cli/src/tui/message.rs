//! Every event that can change the model: an input key (already low-level from
//! crossterm), a tick, or the result of an async effect. `update` is the only
//! consumer.

use crossterm::event::KeyEvent;

pub enum Msg {
    /// First message: kick off the initial data load.
    Init,
    /// A key press from the input thread.
    Key(KeyEvent),
    /// Terminal resized — force a redraw (no state change).
    Redraw,
    /// Animation/idle tick (unused while nothing animates).
    Tick,
    /// Result of `Effect::LoadAll`: cache sizes for the three screens.
    Loaded {
        registry: usize,
        agents: usize,
        sources: usize,
        installed: usize,
    },
    /// An effect failed; surface the message in the status line. (No fallible
    /// effect exists until Phase 2+; kept so the failure path is already wired.)
    #[allow(dead_code)]
    Error(String),
}
