use std::io::{self, IsTerminal};

use serde_json::{json, Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct CliError {
    pub code: String,
    pub message: String,
    pub details: Map<String, Value>,
    json_safe: bool,
}

impl CliError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Map::new(),
            json_safe: true,
        }
    }

    /// Preserve the full diagnostic for the local human-readable path while
    /// preventing parser input, configuration fragments, paths, or backend
    /// error details from crossing the machine-output boundary.
    pub fn private(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Map::new(),
            json_safe: false,
        }
    }

    pub fn redact_json(mut self) -> Self {
        self.json_safe = false;
        self
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    pub fn from_legacy(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = message
            .split_once(':')
            .map(|(prefix, _)| prefix.trim())
            .filter(|prefix| {
                !prefix.is_empty()
                    && prefix
                        .chars()
                        .all(|character| character.is_ascii_lowercase() || character == '_')
            })
            .unwrap_or("operation_failed")
            .to_string();
        Self::private(code, message)
    }

    pub fn from_core(error: mux_core::domain::error::CoreError) -> Self {
        let mut details = Map::new();
        details.extend(error.details);
        if let Some(retry_at) = error.retry_at {
            details.insert("retry_at".into(), Value::String(retry_at));
        }
        if let Some(confirmation) = error.confirmation {
            details.insert(
                "confirmation".into(),
                json!({"kind": confirmation.kind, "token": confirmation.token}),
            );
        }
        Self {
            code: error.code,
            message: error.message,
            details,
            json_safe: false,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CliError {}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutput {
    pub command: &'static str,
    pub changed: bool,
    pub data: Value,
    pub human: String,
}

impl CommandOutput {
    pub fn new(
        command: &'static str,
        changed: bool,
        data: Value,
        human: impl Into<String>,
    ) -> Self {
        Self {
            command,
            changed,
            data,
            human: human.into(),
        }
    }

    pub fn envelope(&self) -> Value {
        json!({
            "schema_version": 1,
            "ok": true,
            "command": self.command,
            "changed": self.changed,
            "data": self.data,
        })
    }
}

pub fn error_envelope(error: &CliError) -> Value {
    let mut error_value = Map::new();
    error_value.insert("code".into(), Value::String(error.code.clone()));
    let message = if error.json_safe {
        error.message.clone()
    } else {
        format!("request failed ({})", error.code)
    };
    error_value.insert("message".into(), Value::String(message));
    if error.json_safe && !error.details.is_empty() {
        error_value.insert("details".into(), Value::Object(error.details.clone()));
    }
    json!({
        "schema_version": 1,
        "ok": false,
        "error": error_value,
    })
}

pub fn render_success(output: &CommandOutput, json_mode: bool) -> Result<(), CliError> {
    if json_mode {
        let encoded = serde_json::to_string_pretty(&output.envelope())
            .map_err(|error| CliError::private("serialization", error.to_string()))?;
        println!("{encoded}");
    } else if !output.human.is_empty() {
        println!("{}", output.human);
    }
    Ok(())
}

pub fn render_error(error: &CliError, json_mode: bool, no_color: bool) {
    if json_mode {
        match serde_json::to_string_pretty(&error_envelope(error)) {
            Ok(encoded) => eprintln!("{encoded}"),
            Err(_) => eprintln!(
                "{{\"schema_version\":1,\"ok\":false,\"error\":{{\"code\":\"serialization\",\"message\":\"failed to encode CLI error\"}}}}"
            ),
        }
    } else {
        eprintln!("{}", Palette::for_stderr(no_color).red(&error.to_string()));
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    enabled: bool,
}

impl Palette {
    pub fn new(no_color: bool) -> Self {
        Self {
            enabled: !no_color && io::stdout().is_terminal(),
        }
    }

    pub fn for_stderr(no_color: bool) -> Self {
        Self {
            enabled: !no_color && io::stderr().is_terminal(),
        }
    }

    fn paint(self, code: &str, value: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }

    pub fn bold(self, value: &str) -> String {
        self.paint("1", value)
    }

    pub fn dim(self, value: &str) -> String {
        self.paint("2", value)
    }

    pub fn green(self, value: &str) -> String {
        self.paint("32", value)
    }

    pub fn yellow(self, value: &str) -> String {
        self.paint("33", value)
    }

    pub fn red(self, value: &str) -> String {
        self.paint("31", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_envelope_has_stable_schema() {
        let value = CommandOutput::new("skill.list", false, json!(["one"]), "one").envelope();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["ok"], true);
        assert_eq!(value["command"], "skill.list");
        assert_eq!(value["changed"], false);
        assert_eq!(value["data"], json!(["one"]));
    }

    #[test]
    fn error_envelope_omits_empty_details() {
        let value = error_envelope(&CliError::new("bad_input", "bad"));
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "bad_input");
        assert!(value["error"].get("details").is_none());
    }

    #[test]
    fn no_color_palette_never_emits_ansi() {
        let palette = Palette::new(true);
        for rendered in [
            palette.bold("x"),
            palette.dim("x"),
            palette.green("x"),
            palette.yellow("x"),
            palette.red("x"),
        ] {
            assert!(!rendered.contains("\x1b["));
        }
    }

    #[test]
    fn legacy_error_preserves_stable_prefix() {
        let error = CliError::from_legacy("model_profile_missing: work");
        assert_eq!(error.code, "model_profile_missing");
        assert_eq!(error.message, "model_profile_missing: work");
    }

    #[test]
    fn private_error_keeps_local_diagnostic_out_of_json() {
        let error = CliError::private(
            "settings_invalid",
            "invalid token SECRET_SENTINEL at /private/user/settings.json",
        )
        .with_detail("raw", "DETAIL_SENTINEL");
        let encoded = error_envelope(&error).to_string();
        assert!(encoded.contains("settings_invalid"));
        assert!(!encoded.contains("SECRET_SENTINEL"));
        assert!(!encoded.contains("/private/user"));
        assert!(!encoded.contains("DETAIL_SENTINEL"));
        assert!(error.to_string().contains("SECRET_SENTINEL"));
    }
}
