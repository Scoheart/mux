use super::files::validate_candidate_anchored_private;
use super::source::{
    check_github_revision, open_recorded_local_skill, validate_github_revision_source,
    GithubRevisionStatus,
};
use super::transaction::acquire_skills_lock;
use super::{
    capped_message, GithubEndpoints, SkillError, SkillSource, SkillsPaths, UpdateCheckOutcome,
};
use crate::settings::{load_settings_strict, mutate_settings};
use chrono::{DateTime, Duration, SecondsFormat, Utc};

const UPDATE_INTERVAL_HOURS: i64 = 24;

#[derive(Debug)]
enum ProbeResult {
    Pinned(Result<(), SkillError>),
    Github(Result<GithubRevisionStatus, SkillError>),
    Local(Result<String, SkillError>),
}

#[derive(Debug)]
struct Probe {
    name: String,
    source: SkillSource,
    resolved_revision: Option<String>,
    result: ProbeResult,
}

pub fn check_updates(manual: bool) -> Result<UpdateCheckOutcome, SkillError> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    check_updates_with(manual, &now, GithubEndpoints::production())
}

pub fn check_updates_if_due() -> Result<UpdateCheckOutcome, SkillError> {
    check_updates(false)
}

#[doc(hidden)]
pub fn check_updates_with(
    manual: bool,
    now: &str,
    endpoints: GithubEndpoints,
) -> Result<UpdateCheckOutcome, SkillError> {
    let now_parsed = DateTime::parse_from_rfc3339(now).map_err(|_| SkillError::InvalidSource {
        message: "the update-check clock is not a valid RFC 3339 timestamp".into(),
    })?;
    let settings = load_settings_strict().map_err(settings_read_error)?;
    if !manual
        && settings
            .skill_update_checked_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .is_some_and(|previous| {
                now_parsed.signed_duration_since(previous) < Duration::hours(UPDATE_INTERVAL_HOURS)
            })
    {
        return Ok(UpdateCheckOutcome {
            performed: false,
            checked: 0,
            available: Vec::new(),
            skipped_pinned: Vec::new(),
            errors: Default::default(),
            checked_at: settings.skill_update_checked_at,
        });
    }

    let read_paths = SkillsPaths::resolve_from_env()?;
    let probes = settings
        .managed_skills
        .unwrap_or_default()
        .into_iter()
        .map(|(name, record)| {
            let result = match &record.source {
                SkillSource::Github { pinned: true, .. } => {
                    ProbeResult::Pinned(validate_github_revision_source(&record.source))
                }
                SkillSource::Imported { .. } => ProbeResult::Pinned(Ok(())),
                SkillSource::Github { .. } => ProbeResult::Github(check_github_revision(
                    &record.source,
                    record.update.etag.as_deref(),
                    &endpoints,
                )),
                SkillSource::Local { .. } => {
                    ProbeResult::Local(local_source_hash(&read_paths, &record.source, &name))
                }
            };
            Probe {
                name,
                source: record.source,
                resolved_revision: record.resolved_revision,
                result,
            }
        })
        .collect::<Vec<_>>();

    // Network and local-tree reads above intentionally happen without the
    // cross-process operation lock. The lock only protects the short compare
    // and persist phase, where settings are re-read by mutate_settings.
    let paths = SkillsPaths::from_env()?;
    let _lock = acquire_skills_lock(&paths)?;
    let mut outcome = UpdateCheckOutcome {
        performed: true,
        checked: 0,
        available: Vec::new(),
        skipped_pinned: Vec::new(),
        errors: Default::default(),
        checked_at: Some(now.to_owned()),
    };
    mutate_settings(|current| {
        let records = current.managed_skills.get_or_insert_default();
        for probe in probes {
            let Some(record) = records.get_mut(&probe.name) else {
                continue;
            };
            if record.source != probe.source || record.resolved_revision != probe.resolved_revision
            {
                continue;
            }
            match probe.result {
                ProbeResult::Pinned(Ok(())) => outcome.skipped_pinned.push(probe.name),
                ProbeResult::Pinned(Err(error)) => {
                    record_probe_error(&mut outcome, &probe.name, &mut record.update, error, now)
                }
                ProbeResult::Github(result) => {
                    outcome.checked += 1;
                    match result {
                        Ok(GithubRevisionStatus::NotModified { etag }) => {
                            record.update.checked_at = Some(now.to_owned());
                            record.update.etag = etag.or_else(|| record.update.etag.clone());
                            record.update.error = None;
                            record.update.retry_at = None;
                            if record.update.available {
                                outcome.available.push(probe.name);
                            }
                        }
                        Ok(GithubRevisionStatus::Resolved { sha, etag }) => {
                            let available = record.resolved_revision.as_deref() != Some(&sha);
                            record.update.available = available;
                            record.update.checked_at = Some(now.to_owned());
                            record.update.resolved_revision = Some(sha);
                            record.update.etag = etag.or_else(|| record.update.etag.clone());
                            record.update.error = None;
                            record.update.retry_at = None;
                            if available {
                                outcome.available.push(probe.name);
                            }
                        }
                        Err(error) => record_probe_error(
                            &mut outcome,
                            &probe.name,
                            &mut record.update,
                            error,
                            now,
                        ),
                    }
                }
                ProbeResult::Local(result) => {
                    outcome.checked += 1;
                    match result {
                        Ok(hash) => {
                            let available = record.content_hash != hash;
                            record.update.available = available;
                            record.update.checked_at = Some(now.to_owned());
                            record.update.resolved_revision = Some(hash);
                            record.update.etag = None;
                            record.update.error = None;
                            record.update.retry_at = None;
                            if available {
                                outcome.available.push(probe.name);
                            }
                        }
                        Err(error) => record_probe_error(
                            &mut outcome,
                            &probe.name,
                            &mut record.update,
                            error,
                            now,
                        ),
                    }
                }
            }
        }
        current.skill_update_checked_at = Some(now.to_owned());
    })
    .map_err(settings_write_error)?;

    outcome.available.sort();
    outcome.skipped_pinned.sort();
    Ok(outcome)
}

fn local_source_hash(
    paths: &SkillsPaths,
    source: &SkillSource,
    expected_name: &str,
) -> Result<String, SkillError> {
    let SkillSource::Local { .. } = source else {
        return Err(SkillError::InvalidSource {
            message: "the recorded local Skill source is invalid".into(),
        });
    };
    let candidate = open_recorded_local_skill(paths, source)?;
    let validated = validate_candidate_anchored_private(&candidate)?;
    if validated.manifest.name != expected_name {
        return Err(SkillError::InvalidSource {
            message: "the recorded local Skill name no longer matches its source".into(),
        });
    }
    Ok(validated.content_hash)
}

fn record_probe_error(
    outcome: &mut UpdateCheckOutcome,
    name: &str,
    state: &mut super::SkillUpdateState,
    error: SkillError,
    now: &str,
) {
    let (message, retry_at) = display_probe_error(error);
    state.checked_at = Some(now.to_owned());
    state.error = Some(message.clone());
    state.retry_at = retry_at;
    outcome.errors.insert(name.to_owned(), message);
}

fn display_probe_error(error: SkillError) -> (String, Option<String>) {
    match error {
        SkillError::Network { message, retry_at } => (capped_message(message), retry_at),
        SkillError::InvalidSource { message }
        | SkillError::PlanStale { message }
        | SkillError::RecoveryRequired { message } => (capped_message(message), None),
        SkillError::InvalidManifest { message, .. }
        | SkillError::UnsafePath { message, .. }
        | SkillError::Conflict { message, .. }
        | SkillError::Io { message, .. } => (capped_message(message), None),
        SkillError::LimitExceeded {
            limit,
            actual,
            allowed,
        } => (
            capped_message(format!("{limit} limit exceeded: {actual} > {allowed}")),
            None,
        ),
        SkillError::ConfirmationRequired { message, .. } => (capped_message(message), None),
    }
}

fn settings_read_error(_error: std::io::Error) -> SkillError {
    SkillError::Io {
        message: "MUX settings could not be read safely".into(),
        path: None,
    }
}

fn settings_write_error(_error: std::io::Error) -> SkillError {
    SkillError::Io {
        message: "Skills update state could not be saved safely".into(),
        path: None,
    }
}
