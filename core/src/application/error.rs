use crate::domain::error::{CoreConfirmation, CoreError};
use std::collections::BTreeMap;

pub(crate) fn from_legacy(message: String) -> CoreError {
    let (code, body) = message
        .split_once(':')
        .filter(|(prefix, _)| {
            !prefix.is_empty()
                && prefix
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '_')
        })
        .map(|(code, body)| (code, body.trim_start()))
        .unwrap_or(("operation_failed", message.as_str()));
    CoreError::new(code, body)
}

pub(crate) fn from_skill(error: crate::resources::skill::SkillError) -> CoreError {
    let parts = error.into_command_parts();
    CoreError {
        code: parts.code.into(),
        message: parts.message,
        details: BTreeMap::new(),
        retry_at: parts.retry_at,
        confirmation: parts.findings_hash.map(|token| {
            Box::new(CoreConfirmation {
                kind: "skill_findings".into(),
                token,
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_codes_are_promoted_without_parsing_arbitrary_messages() {
        assert_eq!(from_legacy("plan_stale: changed".into()).code, "plan_stale");
        assert_eq!(
            from_legacy("Path changed: retry".into()).code,
            "operation_failed"
        );
    }
}
