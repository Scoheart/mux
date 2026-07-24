//! Frontend-independent UI preferences.

pub use crate::pinned_agents::MAX_PINNED_AGENTS;
use crate::settings::{load_settings_strict, mutate_settings_checked, UiSettings};

pub const SUPPORTED_UI_LOCALES: [&str; 2] = ["zh-CN", "en-US"];

pub fn get_pinned_agents() -> Result<Vec<String>, String> {
    super::gate::read(crate::pinned_agents::get_pinned_agents)
}

pub fn set_pinned_agents(ids: Vec<String>) -> Result<Vec<String>, String> {
    super::gate::write(|| crate::pinned_agents::set_pinned_agents(ids))
}

pub fn get_ui_locale() -> Result<Option<String>, String> {
    super::gate::read(|| {
        load_settings_strict()
            .map(|settings| settings.ui.and_then(|ui| ui.locale))
            .map_err(|error| error.to_string())
    })
}

pub fn set_ui_locale(locale: Option<String>) -> Result<Option<String>, String> {
    super::gate::write(|| {
        let locale = normalize_locale(locale)?;
        mutate_settings_checked(move |settings| {
            settings.ui.get_or_insert_with(UiSettings::default).locale = locale.clone();
            Ok(locale)
        })
        .map_err(|error| error.to_string())
    })
}

fn normalize_locale(locale: Option<String>) -> Result<Option<String>, String> {
    let Some(locale) = locale else {
        return Ok(None);
    };
    let locale = locale.trim();
    if SUPPORTED_UI_LOCALES.contains(&locale) {
        Ok(Some(locale.to_string()))
    } else {
        Err(format!("unsupported UI locale: {locale}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testenv::TestHome;
    use serde_json::Value;
    use std::fs;

    #[test]
    fn locale_roundtrips_and_preserves_unknown_ui_fields() {
        let home = TestHome::new("ui-locale-roundtrip");
        let path = home.home.join(".mux/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"ui":{"future_ui_key":{"keep":true}},"future_section":{"keep":true}}"#,
        )
        .unwrap();

        assert_eq!(get_ui_locale().unwrap(), None);
        assert_eq!(
            set_ui_locale(Some("en-US".into())).unwrap().as_deref(),
            Some("en-US")
        );
        assert_eq!(get_ui_locale().unwrap().as_deref(), Some("en-US"));

        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["ui"]["future_ui_key"]["keep"], true);
        assert_eq!(value["future_section"]["keep"], true);
        assert_eq!(value["ui"]["locale"], "en-US");

        assert_eq!(set_ui_locale(None).unwrap(), None);
        let value: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(value["ui"].get("locale").is_none());
    }

    #[test]
    fn locale_rejects_unknown_values_without_writing() {
        let _home = TestHome::new("ui-locale-invalid");
        assert!(set_ui_locale(Some("fr-FR".into())).is_err());
        assert_eq!(get_ui_locale().unwrap(), None);
    }
}
