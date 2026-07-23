//! Frontend-independent UI preferences.

pub use crate::pinned_agents::MAX_PINNED_AGENTS;

pub fn get_pinned_agents() -> Result<Vec<String>, String> {
    super::gate::read(crate::pinned_agents::get_pinned_agents)
}

pub fn set_pinned_agents(ids: Vec<String>) -> Result<Vec<String>, String> {
    super::gate::write(|| crate::pinned_agents::set_pinned_agents(ids))
}
