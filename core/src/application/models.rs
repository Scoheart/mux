//! Model Profile use cases.

pub use crate::domain::types::{ModelProfile, ModelProtocol};
pub use crate::resources::model::{ModelAgentView, ModelProfileView, ModelProviderView};

pub fn list_profiles() -> Vec<ModelProfileView> {
    super::gate::read(crate::resources::model::list_profiles)
}

pub fn list_providers() -> &'static [ModelProviderView] {
    crate::resources::model::list_providers()
}

/// Return Model target capabilities for frontend presentation.
pub fn list_agent_capabilities() -> Result<Vec<ModelAgentView>, String> {
    Ok(super::gate::read(crate::resources::model::list_agents))
}
