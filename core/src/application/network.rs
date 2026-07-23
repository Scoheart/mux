//! Shared network preference use cases.

pub use crate::settings::NetworkSettings;

pub fn get_proxy_settings() -> Result<NetworkSettings, String> {
    super::gate::read(crate::network::get_proxy_settings)
}

pub fn set_proxy_url(proxy_url: Option<String>) -> Result<NetworkSettings, String> {
    super::gate::write(|| crate::network::set_proxy_url(proxy_url))
}
