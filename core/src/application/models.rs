//! Model Profile use cases.

pub use crate::domain::assets::ApiKeyDelivery;
pub use crate::domain::types::{ModelProfile, ModelProtocol};
pub use crate::resources::model::credential::CredentialValidationView;
pub use crate::resources::model::{
    ModelAgentView, ModelProfileView, ModelProviderInstanceView, ModelProviderView,
    ProviderModelSummary,
};

pub fn list_profiles() -> Vec<ModelProfileView> {
    super::gate::read(crate::resources::model::list_profiles)
}

pub fn list_providers() -> &'static [ModelProviderView] {
    crate::resources::model::list_providers()
}

pub fn list_provider_instances() -> Vec<ModelProviderInstanceView> {
    super::gate::read(crate::resources::model::list_provider_instances)
}

pub fn discover_provider_models(provider_id: &str) -> Result<Vec<ProviderModelSummary>, String> {
    super::gate::read(|| crate::resources::model::discover_provider_models(provider_id))
}

pub fn reveal_provider_credential(provider_id: &str) -> Result<String, String> {
    super::gate::read(|| crate::resources::model::reveal_provider_credential(provider_id))
}

pub fn infer_provider(base_url: &str) -> String {
    crate::resources::model::infer_provider(base_url)
}

pub fn validate_credential_source(
    source: &crate::domain::types::ApiKeySource,
) -> Result<CredentialValidationView, String> {
    super::gate::read(|| crate::resources::model::credential::validate_for_ui(source))
}

/// Return Model target capabilities for frontend presentation.
pub fn list_agent_capabilities() -> Result<Vec<ModelAgentView>, String> {
    Ok(super::gate::read(crate::resources::model::list_agents))
}

pub fn set_credential_delivery(
    agent_id: &str,
    profile_id: &str,
    delivery: ApiKeyDelivery,
    confirm_plaintext: bool,
) -> Result<crate::resources::model::ModelApplyResult, String> {
    super::gate::write(|| {
        crate::resources::model::set_model_credential_delivery(
            agent_id,
            profile_id,
            delivery,
            confirm_plaintext,
        )
    })
}
