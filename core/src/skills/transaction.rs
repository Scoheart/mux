use super::{
    copy_tree_secure, hash_tree, io_error, validate_candidate, DirectoryMutation, LinkMutation,
    LinkState, SkillError, SkillSettingsSnapshot, SkillSource, SkillsPaths, TransactionOrder,
    TransactionSpec,
};
use crate::agents::builtin_agents;
use crate::settings::{load_settings_strict, mutate_settings, Settings};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(25);
const MAX_JOURNAL_BYTES: u64 = 4 * 1024 * 1024;
const STAGING_METADATA_BYTES: u64 = 4096;
const STALE_STAGING_AGE_HOURS: i64 = 24;
const STAGING_METADATA_FILE: &str = "metadata.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failpoint {
    AfterFirstLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashPoint {
    AfterContentBeforePhase,
    AfterLinksBeforePhase,
    AfterSettingsBeforePhase,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalPhase {
    Prepared,
    ContentSwapped,
    LinksSwapped,
    SettingsWritten,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Journal {
    spec: TransactionSpec,
    phase: JournalPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JournalWriteFailpoint {
    BeforeRename,
    AfterRenameBeforeParentSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryDisposition {
    RollBack,
    FinishCommit,
}

#[derive(Debug)]
pub(crate) struct SkillsOperationLock(File);

impl Drop for SkillsOperationLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

pub(crate) fn validate_operation_id(value: &str) -> Result<Uuid, SkillError> {
    let parsed = Uuid::parse_str(value).map_err(|_| SkillError::InvalidSource {
        message: "the Skills operation id is invalid".into(),
    })?;
    if parsed.hyphenated().to_string() != value {
        return Err(SkillError::InvalidSource {
            message: "the Skills operation id is invalid".into(),
        });
    }
    Ok(parsed)
}

pub(crate) fn acquire_skills_lock(paths: &SkillsPaths) -> Result<SkillsOperationLock, SkillError> {
    acquire_skills_lock_with_timeout(paths, LOCK_TIMEOUT)
}

fn acquire_skills_lock_with_timeout(
    paths: &SkillsPaths,
    timeout: Duration,
) -> Result<SkillsOperationLock, SkillError> {
    fs::create_dir_all(paths.mux_dir()).map_err(|error| io_error(paths.mux_dir(), error))?;
    let file = open_private_lock(&paths.skills_lock())?;
    let started = Instant::now();
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(SkillsOperationLock(file)),
            Err(error) if error.kind() == ErrorKind::WouldBlock && started.elapsed() < timeout => {
                thread::sleep(LOCK_RETRY_DELAY.min(timeout.saturating_sub(started.elapsed())));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                return Err(SkillError::Conflict {
                    message: "another Skills operation is still running".into(),
                    path: String::new(),
                });
            }
            Err(error) => return Err(io_error(&paths.skills_lock(), error)),
        }
    }
}

#[cfg(unix)]
fn open_private_lock(path: &Path) -> Result<File, SkillError> {
    use rustix::fs::{openat, Mode, OFlags, CWD};

    let descriptor = openat(
        CWD,
        path,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::from(0o600),
    )
    .map_err(|error| io_error(path, error.into()))?;
    let file = File::from(descriptor);
    let metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(SkillError::UnsafePath {
            message: "the Skills operation lock is not a private regular file".into(),
            path: String::new(),
        });
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| io_error(path, error))?;
    Ok(file)
}

#[cfg(not(unix))]
fn open_private_lock(path: &Path) -> Result<File, SkillError> {
    let metadata = fs::symlink_metadata(path).ok();
    if metadata
        .as_ref()
        .is_some_and(|metadata| !metadata.file_type().is_file())
    {
        return Err(SkillError::UnsafePath {
            message: "the Skills operation lock is not a private regular file".into(),
            path: String::new(),
        });
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .map_err(|error| io_error(path, error))
}

fn journal_path(paths: &SkillsPaths, operation_id: &str) -> Result<PathBuf, SkillError> {
    validate_operation_id(operation_id)?;
    Ok(paths
        .journals_skills_dir()
        .join(format!("{operation_id}.json")))
}

fn journal_temp_path(paths: &SkillsPaths, operation_id: &str) -> Result<PathBuf, SkillError> {
    validate_operation_id(operation_id)?;
    Ok(paths
        .journals_skills_dir()
        .join(format!(".{operation_id}.json.tmp")))
}

fn write_journal(
    paths: &SkillsPaths,
    spec: &TransactionSpec,
    phase: JournalPhase,
) -> Result<(), SkillError> {
    write_journal_with_failpoint(paths, spec, phase, None)
}

fn write_journal_with_failpoint(
    paths: &SkillsPaths,
    spec: &TransactionSpec,
    phase: JournalPhase,
    failpoint: Option<JournalWriteFailpoint>,
) -> Result<(), SkillError> {
    validate_operation_id(&spec.operation_id)?;
    create_private_journal_root(paths)?;
    let destination = journal_path(paths, &spec.operation_id)?;
    let temporary = journal_temp_path(paths, &spec.operation_id)?;
    remove_abandoned_journal_temp(&temporary)?;
    let bytes = serde_json::to_vec(&Journal {
        spec: spec.clone(),
        phase,
    })
    .map_err(|error| SkillError::InvalidSource {
        message: format!("the Skills journal could not be encoded: {error}"),
    })?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(SkillError::LimitExceeded {
            limit: "skills_journal".into(),
            actual: bytes.len() as u64,
            allowed: MAX_JOURNAL_BYTES,
        });
    }
    let mut file = create_private_new_file(&temporary)?;
    let before_rename = (|| {
        file.write_all(&bytes)
            .map_err(|error| io_error(&temporary, error))?;
        file.sync_all()
            .map_err(|error| io_error(&temporary, error))?;
        if failpoint == Some(JournalWriteFailpoint::BeforeRename) {
            return Err(SkillError::Io {
                message: "test journal failure before rename".into(),
                path: None,
            });
        }
        fs::rename(&temporary, &destination).map_err(|error| io_error(&destination, error))?;
        Ok(())
    })();
    drop(file);
    if let Err(error) = before_rename {
        remove_abandoned_journal_temp(&temporary)?;
        return Err(error);
    }
    if failpoint == Some(JournalWriteFailpoint::AfterRenameBeforeParentSync) {
        return Err(SkillError::Io {
            message: "test journal failure after rename".into(),
            path: None,
        });
    }
    sync_directory(&paths.journals_skills_dir())
}

fn create_private_journal_root(paths: &SkillsPaths) -> Result<(), SkillError> {
    let root = paths.journals_skills_dir();
    fs::create_dir_all(&root).map_err(|error| io_error(&root, error))?;
    let metadata = fs::symlink_metadata(&root).map_err(|error| io_error(&root, error))?;
    if !metadata.file_type().is_dir() {
        return Err(SkillError::UnsafePath {
            message: "the Skills journal root is not a directory".into(),
            path: String::new(),
        });
    }
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .map_err(|error| io_error(&root, error))?;
    Ok(())
}

#[cfg(unix)]
fn create_private_new_file(path: &Path) -> Result<File, SkillError> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn create_private_new_file(path: &Path) -> Result<File, SkillError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error(path, error))
}

fn remove_abandoned_journal_temp(path: &Path) -> Result<(), SkillError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
        Ok(metadata) if metadata.file_type().is_file() => {
            fs::remove_file(path).map_err(|error| io_error(path, error))
        }
        Ok(_) => Err(SkillError::RecoveryRequired {
            message: "a Skills journal temporary path requires manual recovery".into(),
        }),
    }
}

fn read_journal(path: &Path) -> Result<Journal, SkillError> {
    let mut file = open_read_nofollow(path).map_err(|_| SkillError::RecoveryRequired {
        message: "a Skills journal could not be opened safely".into(),
    })?;
    let metadata = file.metadata().map_err(|_| SkillError::RecoveryRequired {
        message: "a Skills journal could not be inspected safely".into(),
    })?;
    if !metadata.is_file() {
        return Err(SkillError::RecoveryRequired {
            message: "a Skills journal is not a regular file".into(),
        });
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 || metadata.nlink() != 1 {
        return Err(SkillError::RecoveryRequired {
            message: "a Skills journal is not private".into(),
        });
    }
    if metadata.len() > MAX_JOURNAL_BYTES {
        return Err(SkillError::RecoveryRequired {
            message: "a Skills journal exceeds its local size limit".into(),
        });
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_JOURNAL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SkillError::RecoveryRequired {
            message: "a Skills journal could not be read safely".into(),
        })?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(SkillError::RecoveryRequired {
            message: "a Skills journal exceeds its local size limit".into(),
        });
    }
    serde_json::from_slice(&bytes).map_err(|_| SkillError::RecoveryRequired {
        message: "a Skills journal is malformed".into(),
    })
}

#[cfg(unix)]
fn open_read_nofollow(path: &Path) -> std::io::Result<File> {
    use rustix::fs::{openat, Mode, OFlags, CWD};

    openat(
        CWD,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}

#[cfg(not(unix))]
fn open_read_nofollow(path: &Path) -> std::io::Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "refusing to follow a symlink",
        ));
    }
    OpenOptions::new().read(true).open(path)
}

fn sync_directory(path: &Path) -> Result<(), SkillError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error(path, error))
}

pub fn has_pending_recovery() -> Result<bool, SkillError> {
    let paths = SkillsPaths::resolve_from_env()?;
    has_pending_recovery_with_paths(&paths)
}

fn has_pending_recovery_with_paths(paths: &SkillsPaths) -> Result<bool, SkillError> {
    match fs::symlink_metadata(paths.journals_skills_dir()) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_error(&paths.journals_skills_dir(), error)),
        Ok(metadata) if !metadata.file_type().is_dir() || !private_directory(&metadata) => {
            return Err(SkillError::RecoveryRequired {
                message: "the Skills journal root requires manual recovery".into(),
            });
        }
        Ok(_) => {}
    }
    Ok(fs::read_dir(paths.journals_skills_dir())
        .map_err(|error| io_error(&paths.journals_skills_dir(), error))?
        .next()
        .is_some())
}

pub fn execute_transaction(spec: TransactionSpec) -> Result<(), SkillError> {
    execute_transaction_with_failpoint(spec, None)
}

#[doc(hidden)]
pub fn execute_transaction_with_failpoint(
    spec: TransactionSpec,
    failpoint: Option<Failpoint>,
) -> Result<(), SkillError> {
    let paths = SkillsPaths::from_env()?;
    let _lock = acquire_skills_lock(&paths)?;
    validate_transaction_roots(&paths, false)?;
    if has_pending_recovery_with_paths(&paths)? {
        return Err(SkillError::RecoveryRequired {
            message: "a pending Skills operation must be recovered before committing".into(),
        });
    }
    validate_transaction_bounds(&spec, &paths)?;
    validate_all_preconditions(&spec, &paths)?;
    if let Err(error) = write_journal(&paths, &spec, JournalPhase::Prepared) {
        let path = journal_path(&paths, &spec.operation_id)?;
        let journal_may_exist = !matches!(
            fs::symlink_metadata(&path),
            Err(error) if error.kind() == ErrorKind::NotFound
        );
        return if journal_may_exist {
            Err(SkillError::RecoveryRequired {
                message: "the Skills transaction journal requires recovery".into(),
            })
        } else {
            Err(error)
        };
    }

    let result = apply_transaction(&spec, &paths, failpoint);
    if let Err(primary) = result {
        return finish_failed_transaction(&spec, &paths, primary);
    }
    Ok(())
}

fn validate_transaction_roots(
    paths: &SkillsPaths,
    journal_may_be_missing: bool,
) -> Result<(), SkillError> {
    let mux_root = fs::canonicalize(paths.mux_dir()).map_err(|_| unsafe_transaction_path())?;
    for (root, may_be_missing) in [
        (paths.skills_dir(), false),
        (paths.staging_skills_dir(), false),
        (paths.backups_skills_dir(), false),
        (paths.journals_skills_dir(), journal_may_be_missing),
    ] {
        let metadata = match fs::symlink_metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) if may_be_missing && error.kind() == ErrorKind::NotFound => continue,
            Err(_) => return Err(unsafe_transaction_path()),
        };
        if !metadata.file_type().is_dir() || !private_directory(&metadata) {
            return Err(unsafe_transaction_path());
        }
        let canonical = fs::canonicalize(&root).map_err(|_| unsafe_transaction_path())?;
        if canonical == mux_root || !canonical.starts_with(&mux_root) {
            return Err(unsafe_transaction_path());
        }
    }
    Ok(())
}

fn apply_transaction(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    failpoint: Option<Failpoint>,
) -> Result<(), SkillError> {
    match spec.order {
        TransactionOrder::ContentThenLinks => {
            apply_directories(spec, paths)?;
            write_journal(paths, spec, JournalPhase::ContentSwapped)?;
            apply_links(spec, paths, failpoint)?;
            write_journal(paths, spec, JournalPhase::LinksSwapped)?;
        }
        TransactionOrder::LinksThenContent => {
            apply_links(spec, paths, failpoint)?;
            write_journal(paths, spec, JournalPhase::LinksSwapped)?;
            apply_directories(spec, paths)?;
            write_journal(paths, spec, JournalPhase::ContentSwapped)?;
        }
    }
    write_skill_settings(paths, &spec.settings_before, &spec.settings_after)?;
    write_journal(paths, spec, JournalPhase::SettingsWritten)?;
    finish_successful_transaction(spec, paths)
}

fn finish_failed_transaction(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    primary: SkillError,
) -> Result<(), SkillError> {
    if rollback_transaction(spec, paths).is_err() {
        if validate_committed_cleanup_evidence(spec, paths).is_ok()
            && finish_successful_transaction(spec, paths).is_ok()
        {
            return Ok(());
        }
        return Err(SkillError::RecoveryRequired {
            message: "operation failed and rollback requires recovery".into(),
        });
    }
    if remove_staging_operation(paths, &spec.operation_id).is_err() {
        return Err(SkillError::RecoveryRequired {
            message: "operation rollback completed but staging cleanup requires recovery".into(),
        });
    }
    if remove_journal(paths, &spec.operation_id).is_err() {
        return Err(SkillError::RecoveryRequired {
            message: "operation rollback completed but journal cleanup requires recovery".into(),
        });
    }
    Err(primary)
}

fn validate_transaction_bounds(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_operation_id(&spec.operation_id)?;
    validate_settings_snapshot(&spec.settings_before)?;
    validate_settings_snapshot(&spec.settings_after)?;
    let catalog_targets = verified_catalog_targets(paths)?;
    let mut mutation_paths = BTreeSet::new();
    let mut backup_paths = BTreeSet::new();

    for mutation in &spec.directory_mutations {
        validate_central_destination(&mutation.destination, paths)?;
        validate_skill_name_component(&mutation.destination)?;
        insert_unique(&mut mutation_paths, &mutation.destination)?;
        validate_backup_path(&mutation.backup, paths)?;
        insert_unique(&mut backup_paths, &mutation.backup)?;
        if let Some(replacement) = &mutation.replacement {
            validate_staging_path(replacement, &spec.operation_id, paths)?;
        }
    }
    for mutation in &spec.link_mutations {
        validate_link_path(&mutation.path, &catalog_targets)?;
        validate_skill_name_component(&mutation.path)?;
        insert_unique(&mut mutation_paths, &mutation.path)?;
        if let Some(target) = &mutation.desired_target {
            validate_central_destination(target, paths)?;
            validate_skill_name_component(target)?;
        }
        if let LinkState::ManagedSymlink { target } = &mutation.expected {
            validate_central_destination(target, paths)?;
            validate_skill_name_component(target)?;
        }
        match (&mutation.expected, &mutation.backup) {
            (LinkState::Directory { .. }, Some(backup)) => {
                validate_backup_path(backup, paths)?;
                insert_unique(&mut backup_paths, backup)?;
            }
            (LinkState::Directory { .. }, None) => {
                return Err(SkillError::InvalidSource {
                    message: "a reviewed Skill directory replacement requires a backup".into(),
                });
            }
            (_, Some(_)) => {
                return Err(SkillError::InvalidSource {
                    message: "only a reviewed real Skill directory may have a link backup".into(),
                });
            }
            (_, None) => {}
        }
    }
    Ok(())
}

fn insert_unique(paths: &mut BTreeSet<PathBuf>, path: &Path) -> Result<(), SkillError> {
    if !paths.insert(path.to_path_buf()) {
        return Err(SkillError::InvalidSource {
            message: "a Skills transaction contains a duplicate mutation path".into(),
        });
    }
    Ok(())
}

fn validate_settings_snapshot(snapshot: &SkillSettingsSnapshot) -> Result<(), SkillError> {
    if let Some(records) = &snapshot.managed_skills {
        for (name, record) in records {
            if !valid_skill_name(name) || record.name != *name {
                return Err(SkillError::InvalidSource {
                    message: "a Skills transaction contains invalid managed settings".into(),
                });
            }
        }
    }
    if let Some(assignments) = &snapshot.skill_assignments {
        let catalog_ids = catalog_target_ids()?;
        for (name, target_ids) in assignments {
            if !valid_skill_name(name)
                || target_ids
                    .iter()
                    .any(|target_id| !valid_identity(target_id) || !catalog_ids.contains(target_id))
            {
                return Err(SkillError::InvalidSource {
                    message: "a Skills transaction contains invalid assignment settings".into(),
                });
            }
        }
    }
    Ok(())
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}

fn valid_skill_name(value: &str) -> bool {
    value.len() <= 64 && valid_identity(value)
}

fn catalog_target_ids() -> Result<BTreeSet<String>, SkillError> {
    let mut ids = BTreeSet::new();
    for (agent_id, definition) in builtin_agents() {
        let Some(capability) = definition.skills else {
            continue;
        };
        validate_catalog_capability(
            &agent_id,
            &capability.target_id,
            &capability.docs,
            &capability.evidence,
            &capability.verified_at,
            capability.probes.is_empty(),
        )?;
        ids.insert(capability.target_id);
        for alias in capability.aliases {
            if !valid_identity(&alias.target_id) {
                return Err(invalid_catalog());
            }
            ids.insert(alias.target_id);
        }
    }
    Ok(ids)
}

fn verified_catalog_targets(paths: &SkillsPaths) -> Result<Vec<PathBuf>, SkillError> {
    let mut targets = BTreeSet::new();
    for (agent_id, definition) in builtin_agents() {
        let Some(capability) = definition.skills else {
            continue;
        };
        validate_catalog_capability(
            &agent_id,
            &capability.target_id,
            &capability.docs,
            &capability.evidence,
            &capability.verified_at,
            capability.probes.is_empty(),
        )?;
        register_catalog_target(paths, &mut targets, &capability.global_dir)?;
        for alias in capability.aliases {
            if !valid_identity(&alias.target_id) {
                return Err(invalid_catalog());
            }
            register_catalog_target(paths, &mut targets, &alias.global_dir)?;
        }
    }
    Ok(targets.into_iter().collect())
}

fn validate_catalog_capability(
    agent_id: &str,
    target_id: &str,
    docs: &str,
    evidence: &str,
    verified_at: &str,
    probes_empty: bool,
) -> Result<(), SkillError> {
    if !valid_identity(agent_id)
        || !valid_identity(target_id)
        || docs.trim().is_empty()
        || evidence != "official"
        || verified_at.trim().is_empty()
        || probes_empty
    {
        return Err(invalid_catalog());
    }
    Ok(())
}

fn invalid_catalog() -> SkillError {
    SkillError::InvalidSource {
        message: "the verified Agent Skills catalog is inconsistent".into(),
    }
}

fn register_catalog_target(
    paths: &SkillsPaths,
    targets: &mut BTreeSet<PathBuf>,
    global_dir: &str,
) -> Result<(), SkillError> {
    let expanded = paths.expand_user(global_dir).ok_or_else(invalid_catalog)?;
    if !expanded.is_absolute() {
        return Err(invalid_catalog());
    }
    targets.insert(canonicalize_deepest(&expanded)?);
    Ok(())
}

fn validate_central_destination(path: &Path, paths: &SkillsPaths) -> Result<(), SkillError> {
    validate_no_traversal(path)?;
    let root = paths.skills_dir();
    let Some(parent) = path.parent() else {
        return Err(unsafe_transaction_path());
    };
    if lexical_absolute(parent)? != lexical_absolute(&root)?
        || !path.file_name().is_some_and(valid_path_component)
    {
        return Err(unsafe_transaction_path());
    }
    verify_physical_root_membership(parent, &root)
}

fn validate_staging_path(
    path: &Path,
    operation_id: &str,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_no_traversal(path)?;
    validate_strict_descendant(path, &paths.staging_skills_dir())?;
    let relative = path
        .strip_prefix(paths.staging_skills_dir())
        .map_err(|_| unsafe_transaction_path())?;
    if relative
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
        != Some(operation_id)
    {
        return Err(unsafe_transaction_path());
    }
    verify_physical_root_membership(path, &paths.staging_skills_dir())
}

fn validate_backup_path(path: &Path, paths: &SkillsPaths) -> Result<(), SkillError> {
    validate_no_traversal(path)?;
    validate_strict_descendant(path, &paths.backups_skills_dir())?;
    verify_physical_root_membership(path, &paths.backups_skills_dir())
}

fn validate_link_path(path: &Path, targets: &[PathBuf]) -> Result<(), SkillError> {
    validate_no_traversal(path)?;
    if !path.is_absolute() || !path.file_name().is_some_and(valid_path_component) {
        return Err(unsafe_transaction_path());
    }
    let parent = path.parent().ok_or_else(unsafe_transaction_path)?;
    let canonical_parent = canonicalize_deepest(parent)?;
    if !targets.iter().any(|target| target == &canonical_parent) {
        return Err(SkillError::UnsafePath {
            message: "a Skill link is outside the currently verified Agent targets".into(),
            path: String::new(),
        });
    }
    Ok(())
}

fn validate_no_traversal(path: &Path) -> Result<(), SkillError> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(unsafe_transaction_path());
    }
    Ok(())
}

fn validate_skill_name_component(path: &Path) -> Result<(), SkillError> {
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(valid_skill_name)
    {
        return Err(SkillError::InvalidSource {
            message: "a Skills transaction contains an invalid Skill name".into(),
        });
    }
    Ok(())
}

fn validate_strict_descendant(path: &Path, root: &Path) -> Result<(), SkillError> {
    let path = lexical_absolute(path)?;
    let root = lexical_absolute(root)?;
    let relative = path
        .strip_prefix(&root)
        .map_err(|_| unsafe_transaction_path())?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(unsafe_transaction_path());
    }
    Ok(())
}

fn valid_path_component(value: &std::ffi::OsStr) -> bool {
    value
        .to_str()
        .is_some_and(|value| !value.is_empty() && value != "." && value != "..")
}

fn verify_physical_root_membership(path: &Path, root: &Path) -> Result<(), SkillError> {
    let canonical_root = fs::canonicalize(root).map_err(|error| io_error(root, error))?;
    let canonical_path = canonicalize_deepest(path)?;
    if canonical_path != canonical_root && !canonical_path.starts_with(&canonical_root) {
        return Err(unsafe_transaction_path());
    }
    Ok(())
}

fn unsafe_transaction_path() -> SkillError {
    SkillError::UnsafePath {
        message: "a Skills transaction path is outside its allowed root".into(),
        path: String::new(),
    }
}

fn lexical_absolute(path: &Path) -> Result<PathBuf, SkillError> {
    if !path.is_absolute() {
        return Err(unsafe_transaction_path());
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::RootDir
            | std::path::Component::Prefix(_)
            | std::path::Component::Normal(_) => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err(unsafe_transaction_path());
                }
            }
        }
    }
    Ok(normalized)
}

fn canonicalize_deepest(path: &Path) -> Result<PathBuf, SkillError> {
    let normalized = lexical_absolute(path)?;
    let mut cursor = normalized.as_path();
    let mut missing = Vec::<OsString>::new();
    loop {
        match fs::canonicalize(cursor) {
            Ok(mut resolved) => {
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                if fs::symlink_metadata(cursor).is_ok() {
                    return Err(unsafe_transaction_path());
                }
                let Some(name) = cursor.file_name() else {
                    return Err(unsafe_transaction_path());
                };
                missing.push(name.to_os_string());
                cursor = cursor.parent().ok_or_else(unsafe_transaction_path)?;
            }
            Err(_) => return Err(unsafe_transaction_path()),
        }
    }
}

fn validate_all_preconditions(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_settings_precondition(&spec.settings_before)?;
    for (index, mutation) in spec.directory_mutations.iter().enumerate() {
        validate_directory_precondition(spec, paths, mutation, index)?;
    }
    for mutation in &spec.link_mutations {
        validate_link_precondition(paths, mutation)?;
    }
    Ok(())
}

fn validate_directory_precondition(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &DirectoryMutation,
    index: usize,
) -> Result<(), SkillError> {
    validate_central_destination(&mutation.destination, paths)?;
    let observed = optional_directory_hash(&mutation.destination)?;
    if observed != mutation.expected_before_hash {
        return Err(stale("central Skill content changed after review"));
    }
    ensure_missing(
        &mutation.backup,
        "a Skill transaction backup already exists",
    )?;
    ensure_missing(
        &directory_temp_path(spec, paths, index)?,
        "a Skill transaction temporary directory already exists",
    )?;
    if let Some(replacement) = &mutation.replacement {
        validate_staging_path(replacement, &spec.operation_id, paths)?;
        let validated = validate_candidate(replacement)?;
        let name = mutation
            .destination
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(unsafe_transaction_path)?;
        if validated.manifest.name != name {
            return Err(SkillError::InvalidSource {
                message: "staged Skill content does not match its destination name".into(),
            });
        }
        let expected_hash = spec
            .settings_after
            .managed_skills
            .as_ref()
            .and_then(|records| records.get(name))
            .map(|record| record.content_hash.as_str())
            .ok_or_else(|| SkillError::InvalidSource {
                message: "replacement content is not bound to managed Skills settings".into(),
            })?;
        if validated.content_hash != expected_hash {
            return Err(stale("staged Skill content changed after review"));
        }
    }
    Ok(())
}

fn optional_directory_hash(path: &Path) -> Result<Option<String>, SkillError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(path, error)),
        Ok(metadata) if metadata.file_type().is_dir() => hash_tree(path).map(Some),
        Ok(_) => Err(stale("a reviewed Skill directory changed type")),
    }
}

fn ensure_missing(path: &Path, message: &str) -> Result<(), SkillError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
        Ok(_) => Err(SkillError::RecoveryRequired {
            message: message.into(),
        }),
    }
}

fn stale(message: &str) -> SkillError {
    SkillError::PlanStale {
        message: message.into(),
    }
}

fn current_skill_settings(settings: &Settings) -> SkillSettingsSnapshot {
    SkillSettingsSnapshot {
        managed_skills: settings.managed_skills.clone(),
        skill_assignments: settings.skill_assignments.clone(),
        skill_update_checked_at: settings.skill_update_checked_at.clone(),
    }
}

fn validate_settings_precondition(expected: &SkillSettingsSnapshot) -> Result<(), SkillError> {
    let settings = load_settings_strict().map_err(|error| SkillError::Io {
        message: super::capped_message(error.to_string()),
        path: None,
    })?;
    if current_skill_settings(&settings) != *expected {
        return Err(stale("Skills settings changed after review"));
    }
    Ok(())
}

fn directory_temp_path(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    index: usize,
) -> Result<PathBuf, SkillError> {
    validate_operation_id(&spec.operation_id)?;
    Ok(paths.skills_dir().join(format!(
        ".mux-transaction-{}-{index}.tmp",
        spec.operation_id
    )))
}

fn link_temp_path(spec: &TransactionSpec, mutation: &LinkMutation, index: usize) -> PathBuf {
    mutation
        .path
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .join(format!(
            ".mux-transaction-{}-{index}.link.tmp",
            spec.operation_id
        ))
}

fn apply_directories(spec: &TransactionSpec, paths: &SkillsPaths) -> Result<(), SkillError> {
    for (index, mutation) in spec.directory_mutations.iter().enumerate() {
        apply_directory(spec, paths, mutation, index)?;
    }
    Ok(())
}

fn apply_directory(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &DirectoryMutation,
    index: usize,
) -> Result<(), SkillError> {
    validate_transaction_roots(paths, false)?;
    validate_directory_precondition(spec, paths, mutation, index)?;
    let temporary = directory_temp_path(spec, paths, index)?;
    let replacement_hash = if let Some(replacement) = &mutation.replacement {
        copy_tree_secure(replacement, &temporary)?;
        sync_tree(&temporary)?;
        let source_hash = hash_tree(replacement)?;
        let copied_hash = hash_tree(&temporary)?;
        if copied_hash != source_hash {
            return Err(stale("staged Skill content changed while it was copied"));
        }
        Some(copied_hash)
    } else {
        None
    };
    if replacement_hash != directory_after_hash(spec, mutation)? {
        remove_safe_entry_and_sync(&temporary)?;
        return Err(stale("staged Skill content changed before replacement"));
    }

    if mutation.expected_before_hash.is_some() {
        create_private_parent(&mutation.backup, &paths.backups_skills_dir())?;
    }

    // Copying, syncing, and creating the private backup parent may take long
    // enough for another writer to change reviewed state. This is the final
    // evidence check before the first rename.
    if let Err(error) = validate_directory_swap_precondition(mutation, paths) {
        remove_safe_entry_and_sync(&temporary)?;
        return Err(error);
    }
    if mutation.expected_before_hash.is_some() {
        fs::rename(&mutation.destination, &mutation.backup)
            .map_err(|error| io_error(&mutation.destination, error))?;
        sync_two_parents(&mutation.destination, &mutation.backup)?;
        if optional_directory_hash(&mutation.backup)? != mutation.expected_before_hash
            || optional_directory_hash(&mutation.destination)?.is_some()
        {
            return Err(SkillError::Io {
                message: "the reviewed Skill backup did not persist as expected".into(),
                path: None,
            });
        }
    }
    if mutation.replacement.is_some() {
        validate_central_destination(&mutation.destination, paths)?;
        validate_central_destination(&temporary, paths)?;
        if optional_directory_hash(&mutation.destination)?.is_some()
            || optional_directory_hash(&temporary)? != replacement_hash
        {
            return Err(stale("central Skill content changed before replacement"));
        }
        fs::rename(&temporary, &mutation.destination)
            .map_err(|error| io_error(&mutation.destination, error))?;
        sync_directory(
            mutation
                .destination
                .parent()
                .ok_or_else(unsafe_transaction_path)?,
        )?;
        if optional_directory_hash(&mutation.destination)? != replacement_hash {
            return Err(SkillError::Io {
                message: "the central Skill replacement did not persist as expected".into(),
                path: None,
            });
        }
    }
    Ok(())
}

fn validate_directory_swap_precondition(
    mutation: &DirectoryMutation,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_transaction_roots(paths, false)?;
    validate_central_destination(&mutation.destination, paths)?;
    validate_backup_path(&mutation.backup, paths)?;
    if optional_directory_hash(&mutation.destination)? != mutation.expected_before_hash {
        return Err(stale("central Skill content changed before replacement"));
    }
    ensure_missing(
        &mutation.backup,
        "a Skill transaction backup already exists",
    )
}

fn create_private_parent(path: &Path, root: &Path) -> Result<(), SkillError> {
    let parent = path.parent().ok_or_else(unsafe_transaction_path)?;
    validate_strict_descendant(path, root)?;
    fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| unsafe_transaction_path())?;
    let mut cursor = root.to_path_buf();
    sync_directory(&cursor)?;
    for component in relative.components() {
        let containing_directory = cursor.clone();
        cursor.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&cursor).map_err(|error| io_error(&cursor, error))?;
        if !metadata.file_type().is_dir() {
            return Err(unsafe_transaction_path());
        }
        #[cfg(unix)]
        fs::set_permissions(&cursor, fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error(&cursor, error))?;
        sync_directory(&cursor)?;
        sync_directory(&containing_directory)?;
    }
    Ok(())
}

fn sync_two_parents(left: &Path, right: &Path) -> Result<(), SkillError> {
    let left_parent = left.parent().ok_or_else(unsafe_transaction_path)?;
    let right_parent = right.parent().ok_or_else(unsafe_transaction_path)?;
    sync_directory(left_parent)?;
    if right_parent != left_parent {
        sync_directory(right_parent)?;
    }
    Ok(())
}

fn sync_tree(root: &Path) -> Result<(), SkillError> {
    let metadata = fs::symlink_metadata(root).map_err(|error| io_error(root, error))?;
    if !metadata.file_type().is_dir() {
        return Err(unsafe_transaction_path());
    }
    sync_tree_directory(root)
}

fn sync_tree_directory(path: &Path) -> Result<(), SkillError> {
    let mut entries = fs::read_dir(path)
        .map_err(|error| io_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(path, error))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let child = entry.path();
        let metadata = fs::symlink_metadata(&child).map_err(|error| io_error(&child, error))?;
        if metadata.file_type().is_dir() {
            sync_tree_directory(&child)?;
        } else if metadata.file_type().is_file() {
            File::open(&child)
                .and_then(|file| file.sync_all())
                .map_err(|error| io_error(&child, error))?;
        } else if !metadata.file_type().is_symlink() {
            return Err(unsafe_transaction_path());
        }
    }
    sync_directory(path)
}

fn remove_safe_entry(path: &Path) -> Result<(), SkillError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
        Ok(metadata) if metadata.file_type().is_dir() => {
            fs::remove_dir_all(path).map_err(|error| io_error(path, error))
        }
        Ok(_) => fs::remove_file(path).map_err(|error| io_error(path, error)),
    }
}

fn remove_safe_entry_and_sync(path: &Path) -> Result<(), SkillError> {
    remove_safe_entry(path)?;
    sync_directory(path.parent().ok_or_else(unsafe_transaction_path)?)
}

fn apply_links(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    failpoint: Option<Failpoint>,
) -> Result<(), SkillError> {
    for (index, mutation) in spec.link_mutations.iter().enumerate() {
        apply_link(spec, paths, mutation, index)?;
        if index == 0 && failpoint == Some(Failpoint::AfterFirstLink) {
            return Err(SkillError::Io {
                message: "test failure after the first Skill link".into(),
                path: None,
            });
        }
    }
    Ok(())
}

fn apply_link(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &LinkMutation,
    index: usize,
) -> Result<(), SkillError> {
    validate_transaction_roots(paths, false)?;
    validate_link_runtime_bounds(mutation, paths)?;
    validate_link_precondition(paths, mutation)?;
    let parent = mutation.path.parent().ok_or_else(unsafe_transaction_path)?;
    create_verified_target_root(parent, paths)?;
    validate_link_precondition(paths, mutation)?;
    let temporary = link_temp_path(spec, mutation, index);
    validate_link_path(&temporary, &verified_catalog_targets(paths)?)?;
    ensure_missing(
        &temporary,
        "a Skill link temporary path already exists and requires recovery",
    )?;

    if matches!(mutation.expected, LinkState::Directory { .. }) {
        let backup = mutation
            .backup
            .as_ref()
            .ok_or_else(unsafe_transaction_path)?;
        create_private_parent(backup, &paths.backups_skills_dir())?;
        validate_link_runtime_bounds(mutation, paths)?;
        validate_link_precondition(paths, mutation)?;
        ensure_missing(backup, "a Skill link backup already exists")?;
        fs::rename(&mutation.path, backup).map_err(|error| io_error(&mutation.path, error))?;
        sync_two_parents(&mutation.path, backup)?;
        validate_link_directory_backup(mutation)?;
    }

    match &mutation.desired_target {
        Some(target) => {
            validate_managed_target_exists(target, paths)?;
            create_symlink(target, &temporary)?;
            sync_directory(parent)?;
            if fs::read_link(&temporary).map_err(|error| io_error(&temporary, error))? != *target {
                return Err(stale("a Skill link temporary changed before replacement"));
            }
            validate_link_runtime_bounds(mutation, paths)?;
            if matches!(mutation.expected, LinkState::Directory { .. }) {
                validate_link_directory_backup(mutation)?;
            } else {
                validate_link_precondition(paths, mutation)?;
            }
            fs::rename(&temporary, &mutation.path)
                .map_err(|error| io_error(&mutation.path, error))?;
            sync_directory(parent)?;
        }
        None if matches!(mutation.expected, LinkState::Directory { .. }) => {
            validate_link_runtime_bounds(mutation, paths)?;
            validate_link_directory_backup(mutation)?;
        }
        None => {
            validate_link_runtime_bounds(mutation, paths)?;
            validate_link_precondition(paths, mutation)?;
            match fs::symlink_metadata(&mutation.path) {
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(&mutation.path, error)),
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    fs::remove_file(&mutation.path)
                        .map_err(|error| io_error(&mutation.path, error))?;
                    sync_directory(parent)?;
                }
                Ok(_) => return Err(stale("a reviewed Skill link changed type")),
            }
        }
    }
    Ok(())
}

fn validate_link_runtime_bounds(
    mutation: &LinkMutation,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_transaction_roots(paths, false)?;
    validate_link_path(&mutation.path, &verified_catalog_targets(paths)?)?;
    if let Some(target) = &mutation.desired_target {
        validate_central_destination(target, paths)?;
    }
    if let LinkState::ManagedSymlink { target } = &mutation.expected {
        validate_central_destination(target, paths)?;
    }
    if let Some(backup) = &mutation.backup {
        validate_backup_path(backup, paths)?;
    }
    Ok(())
}

fn validate_managed_target_exists(target: &Path, paths: &SkillsPaths) -> Result<(), SkillError> {
    validate_central_destination(target, paths)?;
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(stale("a managed Skill target changed type")),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Err(stale("a managed Skill target disappeared"))
        }
        Err(error) => Err(io_error(target, error)),
    }
}

fn validate_link_directory_backup(mutation: &LinkMutation) -> Result<(), SkillError> {
    let (LinkState::Directory { tree_hash }, Some(backup)) = (&mutation.expected, &mutation.backup)
    else {
        return Err(recovery_evidence_error());
    };
    if optional_directory_hash_recovery(backup)?.as_deref() != Some(tree_hash.as_str())
        || !matches!(
            fs::symlink_metadata(&mutation.path),
            Err(error) if error.kind() == ErrorKind::NotFound
        )
    {
        return Err(recovery_evidence_error());
    }
    Ok(())
}

fn create_verified_target_root(parent: &Path, paths: &SkillsPaths) -> Result<(), SkillError> {
    let targets = verified_catalog_targets(paths)?;
    let expected = canonicalize_deepest(parent)?;
    if !targets.iter().any(|target| target == &expected) {
        return Err(unsafe_transaction_path());
    }
    if fs::symlink_metadata(parent).is_err() {
        fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    }
    let actual = canonicalize_deepest(parent)?;
    if actual != expected {
        return Err(unsafe_transaction_path());
    }
    for directory in parent.ancestors().take(3) {
        sync_directory(directory)?;
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> Result<(), SkillError> {
    std::os::unix::fs::symlink(target, link).map_err(|error| io_error(link, error))
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) -> Result<(), SkillError> {
    std::os::windows::fs::symlink_dir(target, link).map_err(|error| io_error(link, error))
}

#[cfg(not(any(unix, windows)))]
fn create_symlink(_target: &Path, _link: &Path) -> Result<(), SkillError> {
    Err(SkillError::InvalidSource {
        message: "Skill link transactions are unsupported on this platform".into(),
    })
}

fn validate_link_precondition(
    paths: &SkillsPaths,
    mutation: &LinkMutation,
) -> Result<(), SkillError> {
    if !link_matches_state(&mutation.path, &mutation.expected, paths)? {
        return Err(stale("an Agent Skill target changed after review"));
    }
    if let Some(backup) = &mutation.backup {
        ensure_missing(backup, "a Skill link backup already exists")?;
    }
    Ok(())
}

fn link_matches_state(
    path: &Path,
    expected: &LinkState,
    paths: &SkillsPaths,
) -> Result<bool, SkillError> {
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(matches!(expected, LinkState::Missing));
        }
        Err(error) => return Err(io_error(path, error)),
        Ok(metadata) => metadata,
    };
    match expected {
        LinkState::Missing => Ok(false),
        LinkState::Directory { tree_hash } => {
            if !metadata.file_type().is_dir() {
                return Ok(false);
            }
            Ok(hash_tree(path)? == *tree_hash)
        }
        LinkState::ManagedSymlink { target } => {
            if !metadata.file_type().is_symlink()
                || fs::read_link(path).map_err(|error| io_error(path, error))? != *target
            {
                return Ok(false);
            }
            let target_canonical = match fs::canonicalize(target) {
                Ok(path) => path,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(io_error(target, error)),
            };
            let link_canonical = match fs::canonicalize(path) {
                Ok(path) => path,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(io_error(path, error)),
            };
            let central = fs::canonicalize(paths.skills_dir())
                .map_err(|error| io_error(&paths.skills_dir(), error))?;
            Ok(link_canonical == target_canonical
                && (target_canonical == central || target_canonical.starts_with(central)))
        }
        LinkState::BrokenSymlink { target } => {
            if !metadata.file_type().is_symlink()
                || fs::read_link(path).map_err(|error| io_error(path, error))? != *target
            {
                return Ok(false);
            }
            match fs::metadata(path) {
                Ok(_) => Ok(false),
                Err(error)
                    if error.kind() == ErrorKind::NotFound || is_symlink_loop_error(&error) =>
                {
                    Ok(true)
                }
                Err(error) => Err(io_error(path, error)),
            }
        }
        LinkState::UnknownSymlink { target } => {
            if !metadata.file_type().is_symlink()
                || fs::read_link(path).map_err(|error| io_error(path, error))? != *target
            {
                return Ok(false);
            }
            let resolved = match fs::canonicalize(path) {
                Ok(path) => path,
                Err(error)
                    if error.kind() == ErrorKind::NotFound || is_symlink_loop_error(&error) =>
                {
                    return Ok(false);
                }
                Err(error) => return Err(io_error(path, error)),
            };
            let central = fs::canonicalize(paths.skills_dir())
                .map_err(|error| io_error(&paths.skills_dir(), error))?;
            Ok(resolved != central && !resolved.starts_with(central))
        }
    }
}

#[cfg(unix)]
fn is_symlink_loop_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
}

#[cfg(not(unix))]
fn is_symlink_loop_error(_error: &std::io::Error) -> bool {
    false
}

fn write_skill_settings(
    paths: &SkillsPaths,
    expected: &SkillSettingsSnapshot,
    desired: &SkillSettingsSnapshot,
) -> Result<(), SkillError> {
    validate_settings_precondition(expected)?;
    let applied = mutate_settings(|settings| {
        if current_skill_settings(settings) != *expected {
            return false;
        }
        settings.managed_skills = desired.managed_skills.clone();
        settings.skill_assignments = desired.skill_assignments.clone();
        settings.skill_update_checked_at = desired.skill_update_checked_at.clone();
        true
    })
    .map_err(|error| SkillError::Io {
        message: super::capped_message(error.to_string()),
        path: None,
    })?;
    if !applied {
        return Err(stale("Skills settings changed before they were written"));
    }
    sync_directory(paths.mux_dir())
}

fn restore_skill_settings(
    paths: &SkillsPaths,
    before: &SkillSettingsSnapshot,
    after: &SkillSettingsSnapshot,
) -> Result<(), SkillError> {
    let current = load_settings_strict().map_err(|error| SkillError::Io {
        message: super::capped_message(error.to_string()),
        path: None,
    })?;
    let observed = current_skill_settings(&current);
    if observed == *before {
        return Ok(());
    }
    if observed != *after {
        return Err(SkillError::RecoveryRequired {
            message: "concurrent Skills settings prevent automatic recovery".into(),
        });
    }
    let restored = mutate_settings(|settings| {
        if current_skill_settings(settings) != *after {
            return false;
        }
        settings.managed_skills = before.managed_skills.clone();
        settings.skill_assignments = before.skill_assignments.clone();
        settings.skill_update_checked_at = before.skill_update_checked_at.clone();
        true
    })
    .map_err(|_| SkillError::RecoveryRequired {
        message: "Skills settings could not be restored automatically".into(),
    })?;
    if !restored {
        return Err(SkillError::RecoveryRequired {
            message: "concurrent Skills settings prevent automatic recovery".into(),
        });
    }
    sync_directory(paths.mux_dir())
}

fn rollback_transaction(spec: &TransactionSpec, paths: &SkillsPaths) -> Result<(), SkillError> {
    validate_rollback_evidence(spec, paths)?;
    restore_skill_settings(paths, &spec.settings_before, &spec.settings_after)?;
    match spec.order {
        TransactionOrder::ContentThenLinks => {
            rollback_links(spec, paths)?;
            rollback_directories(spec, paths)?;
        }
        TransactionOrder::LinksThenContent => {
            rollback_directories(spec, paths)?;
            rollback_links(spec, paths)?;
        }
    }
    Ok(())
}

fn validate_rollback_evidence(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_transaction_roots(paths, false)?;
    validate_transaction_bounds(spec, paths)?;
    let settings = load_settings_strict().map_err(|_| recovery_evidence_error())?;
    let observed = current_skill_settings(&settings);
    if observed != spec.settings_before && observed != spec.settings_after {
        return Err(recovery_evidence_error());
    }
    for (index, mutation) in spec.directory_mutations.iter().enumerate() {
        validate_directory_rollback_evidence(spec, paths, mutation, index)?;
    }
    for (index, mutation) in spec.link_mutations.iter().enumerate() {
        validate_link_rollback_evidence(spec, paths, mutation, index)?;
    }
    Ok(())
}

fn validate_directory_rollback_evidence(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &DirectoryMutation,
    index: usize,
) -> Result<(), SkillError> {
    validate_directory_recovery_bounds(spec, paths, mutation, index)?;
    let after_hash = directory_after_hash(spec, mutation)?;
    let temporary = optional_directory_hash_recovery(&directory_temp_path(spec, paths, index)?)?;
    if temporary.is_some() && mutation.replacement.is_none() {
        return Err(recovery_evidence_error());
    }
    let destination = optional_directory_hash_recovery(&mutation.destination)?;
    let backup = optional_directory_hash_recovery(&mutation.backup)?;
    match &mutation.expected_before_hash {
        None => {
            if backup.is_some()
                || (destination.is_some() && destination.as_ref() != after_hash.as_ref())
            {
                return Err(recovery_evidence_error());
            }
        }
        Some(before) if destination.as_deref() == Some(before.as_str()) => {
            if backup.as_ref().is_some_and(|hash| hash != before) {
                return Err(recovery_evidence_error());
            }
        }
        Some(before) => {
            if backup.as_deref() != Some(before.as_str())
                || (destination.is_some() && destination.as_ref() != after_hash.as_ref())
            {
                return Err(recovery_evidence_error());
            }
        }
    }
    Ok(())
}

fn validate_link_rollback_evidence(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &LinkMutation,
    index: usize,
) -> Result<(), SkillError> {
    validate_link_runtime_bounds(mutation, paths)?;
    let temporary = link_temp_path(spec, mutation, index);
    validate_link_path(&temporary, &verified_catalog_targets(paths)?)?;
    validate_link_temporary_evidence(&temporary, mutation)?;
    if link_matches_state(&mutation.path, &mutation.expected, paths)? {
        let (LinkState::Directory { tree_hash }, Some(backup)) =
            (&mutation.expected, &mutation.backup)
        else {
            return Ok(());
        };
        return match optional_directory_hash_recovery(backup)? {
            None => Ok(()),
            Some(observed) if observed == *tree_hash => Ok(()),
            Some(_) => Err(recovery_evidence_error()),
        };
    }
    if !link_is_desired_or_missing(mutation, paths)? {
        return Err(recovery_evidence_error());
    }
    match (&mutation.expected, &mutation.backup) {
        (LinkState::Directory { tree_hash }, Some(backup)) => {
            if optional_directory_hash_recovery(backup)?.as_deref() != Some(tree_hash.as_str()) {
                return Err(recovery_evidence_error());
            }
        }
        (LinkState::Directory { .. }, None) | (_, Some(_)) => {
            return Err(recovery_evidence_error());
        }
        (LinkState::BrokenSymlink { .. } | LinkState::UnknownSymlink { .. }, None) => {
            validate_expected_symlink_recreation(mutation, paths)?
        }
        (_, None) => {}
    }
    Ok(())
}

fn rollback_directories(spec: &TransactionSpec, paths: &SkillsPaths) -> Result<(), SkillError> {
    for (index, mutation) in spec.directory_mutations.iter().enumerate().rev() {
        rollback_directory(spec, paths, mutation, index)?;
    }
    Ok(())
}

fn rollback_directory(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &DirectoryMutation,
    index: usize,
) -> Result<(), SkillError> {
    validate_directory_recovery_bounds(spec, paths, mutation, index)?;
    let after_hash = directory_after_hash(spec, mutation)?;
    let temporary = directory_temp_path(spec, paths, index)?;
    if optional_directory_hash_recovery(&temporary)?.is_some() {
        if mutation.replacement.is_none() {
            return Err(recovery_evidence_error());
        }
        validate_directory_recovery_bounds(spec, paths, mutation, index)?;
        remove_safe_entry_and_sync(&temporary)?;
    }

    let destination_hash = optional_directory_hash_recovery(&mutation.destination)?;
    let backup_hash = optional_directory_hash_recovery(&mutation.backup)?;
    match &mutation.expected_before_hash {
        None => {
            if let Some(observed) = destination_hash {
                if Some(&observed) != after_hash.as_ref() {
                    return Err(recovery_evidence_error());
                }
                validate_directory_recovery_bounds(spec, paths, mutation, index)?;
                remove_safe_entry(&mutation.destination)?;
                sync_directory(&paths.skills_dir())?;
            }
            if backup_hash.is_some() {
                return Err(recovery_evidence_error());
            }
        }
        Some(before_hash) => {
            if destination_hash.as_deref() == Some(before_hash.as_str()) {
                if let Some(observed_backup) = backup_hash {
                    if observed_backup != *before_hash {
                        return Err(recovery_evidence_error());
                    }
                    validate_directory_recovery_bounds(spec, paths, mutation, index)?;
                    remove_safe_entry_and_sync(&mutation.backup)?;
                }
                return Ok(());
            }
            if backup_hash.as_deref() != Some(before_hash.as_str()) {
                return Err(recovery_evidence_error());
            }
            if let Some(observed) = destination_hash {
                if Some(&observed) != after_hash.as_ref() {
                    return Err(recovery_evidence_error());
                }
                validate_directory_recovery_bounds(spec, paths, mutation, index)?;
                remove_safe_entry(&mutation.destination)?;
            }
            validate_directory_recovery_bounds(spec, paths, mutation, index)?;
            if optional_directory_hash_recovery(&mutation.backup)?.as_deref()
                != Some(before_hash.as_str())
                || optional_directory_hash_recovery(&mutation.destination)?.is_some()
            {
                return Err(recovery_evidence_error());
            }
            fs::rename(&mutation.backup, &mutation.destination)
                .map_err(|_| recovery_evidence_error())?;
            sync_two_parents(&mutation.backup, &mutation.destination)?;
            if optional_directory_hash_recovery(&mutation.destination)?.as_deref()
                != Some(before_hash.as_str())
            {
                return Err(recovery_evidence_error());
            }
        }
    }
    Ok(())
}

fn validate_directory_recovery_bounds(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &DirectoryMutation,
    index: usize,
) -> Result<(), SkillError> {
    validate_transaction_roots(paths, false)?;
    validate_central_destination(&mutation.destination, paths)?;
    validate_backup_path(&mutation.backup, paths)?;
    validate_central_destination(&directory_temp_path(spec, paths, index)?, paths)
}

fn directory_after_hash(
    spec: &TransactionSpec,
    mutation: &DirectoryMutation,
) -> Result<Option<String>, SkillError> {
    if mutation.replacement.is_none() {
        return Ok(None);
    }
    let name = mutation
        .destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(recovery_evidence_error)?;
    spec.settings_after
        .managed_skills
        .as_ref()
        .and_then(|records| records.get(name))
        .map(|record| Some(record.content_hash.clone()))
        .ok_or_else(recovery_evidence_error)
}

fn optional_directory_hash_recovery(path: &Path) -> Result<Option<String>, SkillError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(recovery_evidence_error()),
        Ok(metadata) if metadata.file_type().is_dir() => hash_tree(path)
            .map(Some)
            .map_err(|_| recovery_evidence_error()),
        Ok(_) => Err(recovery_evidence_error()),
    }
}

fn recovery_evidence_error() -> SkillError {
    SkillError::RecoveryRequired {
        message: "filesystem evidence does not match the Skills recovery journal".into(),
    }
}

fn rollback_links(spec: &TransactionSpec, paths: &SkillsPaths) -> Result<(), SkillError> {
    for (index, mutation) in spec.link_mutations.iter().enumerate().rev() {
        rollback_link(spec, paths, mutation, index)?;
    }
    Ok(())
}

fn rollback_link(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &LinkMutation,
    index: usize,
) -> Result<(), SkillError> {
    validate_link_runtime_bounds(mutation, paths)?;
    let temporary = link_temp_path(spec, mutation, index);
    validate_link_path(&temporary, &verified_catalog_targets(paths)?)?;
    cleanup_link_temporary(&temporary, mutation)?;
    if link_matches_state(&mutation.path, &mutation.expected, paths)? {
        validate_link_runtime_bounds(mutation, paths)?;
        cleanup_redundant_link_backup(mutation)?;
        return Ok(());
    }

    if let LinkState::Directory { tree_hash } = &mutation.expected {
        let backup = mutation
            .backup
            .as_ref()
            .ok_or_else(recovery_evidence_error)?;
        if optional_directory_hash_recovery(backup)?.as_deref() != Some(tree_hash.as_str()) {
            return Err(recovery_evidence_error());
        }
        if !link_is_desired_or_missing(mutation, paths)? {
            return Err(recovery_evidence_error());
        }
        validate_link_runtime_bounds(mutation, paths)?;
        if !link_is_desired_or_missing(mutation, paths)? {
            return Err(recovery_evidence_error());
        }
        remove_current_link_if_present(&mutation.path)?;
        validate_link_runtime_bounds(mutation, paths)?;
        if optional_directory_hash_recovery(backup)?.as_deref() != Some(tree_hash.as_str())
            || !matches!(
                fs::symlink_metadata(&mutation.path),
                Err(error) if error.kind() == ErrorKind::NotFound
            )
        {
            return Err(recovery_evidence_error());
        }
        fs::rename(backup, &mutation.path).map_err(|_| recovery_evidence_error())?;
        sync_two_parents(backup, &mutation.path)?;
        if !link_matches_state(&mutation.path, &mutation.expected, paths)? {
            return Err(recovery_evidence_error());
        }
        return Ok(());
    }

    if !link_is_desired_or_missing(mutation, paths)? {
        return Err(recovery_evidence_error());
    }
    validate_expected_symlink_recreation(mutation, paths)?;
    validate_link_runtime_bounds(mutation, paths)?;
    if !link_is_desired_or_missing(mutation, paths)? {
        return Err(recovery_evidence_error());
    }
    remove_current_link_if_present(&mutation.path)?;
    match &mutation.expected {
        LinkState::Missing => {}
        LinkState::ManagedSymlink { target }
        | LinkState::BrokenSymlink { target }
        | LinkState::UnknownSymlink { target } => {
            create_symlink_atomic(spec, mutation, index, target, paths)?;
        }
        LinkState::Directory { .. } => unreachable!(),
    }
    if !link_matches_state(&mutation.path, &mutation.expected, paths)? {
        return Err(recovery_evidence_error());
    }
    Ok(())
}

fn link_is_desired_or_missing(
    mutation: &LinkMutation,
    paths: &SkillsPaths,
) -> Result<bool, SkillError> {
    match &mutation.desired_target {
        Some(target) => Ok(link_matches_state(
            &mutation.path,
            &LinkState::ManagedSymlink {
                target: target.clone(),
            },
            paths,
        )? || matches!(
            fs::symlink_metadata(&mutation.path),
            Err(error) if error.kind() == ErrorKind::NotFound
        )),
        None => Ok(matches!(
            fs::symlink_metadata(&mutation.path),
            Err(error) if error.kind() == ErrorKind::NotFound
        )),
    }
}

fn validate_expected_symlink_recreation(
    mutation: &LinkMutation,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    let target = match &mutation.expected {
        LinkState::Missing | LinkState::Directory { .. } => return Ok(()),
        LinkState::ManagedSymlink { target } => {
            validate_central_destination(target, paths)?;
            let metadata = fs::symlink_metadata(target).map_err(|_| recovery_evidence_error())?;
            if !metadata.file_type().is_dir() {
                return Err(recovery_evidence_error());
            }
            return Ok(());
        }
        LinkState::BrokenSymlink { target } | LinkState::UnknownSymlink { target } => target,
    };
    let resolved = resolve_link_target(&mutation.path, target)?;
    match &mutation.expected {
        LinkState::BrokenSymlink { .. } => {
            if resolved
                == lexical_absolute(&mutation.path).map_err(|_| recovery_evidence_error())?
            {
                return Ok(());
            }
            match fs::metadata(&resolved) {
                Err(error)
                    if error.kind() == ErrorKind::NotFound || is_symlink_loop_error(&error) =>
                {
                    Ok(())
                }
                _ => Err(recovery_evidence_error()),
            }
        }
        LinkState::UnknownSymlink { .. } => {
            let resolved = fs::canonicalize(&resolved).map_err(|_| recovery_evidence_error())?;
            let central =
                fs::canonicalize(paths.skills_dir()).map_err(|_| recovery_evidence_error())?;
            if resolved == central || resolved.starts_with(central) {
                return Err(recovery_evidence_error());
            }
            Ok(())
        }
        LinkState::Missing | LinkState::ManagedSymlink { .. } | LinkState::Directory { .. } => {
            unreachable!()
        }
    }
}

fn resolve_link_target(link: &Path, target: &Path) -> Result<PathBuf, SkillError> {
    if target.is_absolute() {
        return lexical_absolute(target).map_err(|_| recovery_evidence_error());
    }
    let parent = link.parent().ok_or_else(recovery_evidence_error)?;
    lexical_absolute(&parent.join(target)).map_err(|_| recovery_evidence_error())
}

fn cleanup_link_temporary(path: &Path, mutation: &LinkMutation) -> Result<(), SkillError> {
    validate_link_temporary_evidence(path, mutation)?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(_) => Err(recovery_evidence_error()),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::remove_file(path).map_err(|_| recovery_evidence_error())?;
            sync_directory(path.parent().ok_or_else(recovery_evidence_error)?)
                .map_err(|_| recovery_evidence_error())
        }
        Ok(_) => Err(recovery_evidence_error()),
    }
}

fn validate_link_temporary_evidence(
    path: &Path,
    mutation: &LinkMutation,
) -> Result<(), SkillError> {
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(recovery_evidence_error()),
        Ok(metadata) => metadata,
    };
    if !metadata.file_type().is_symlink() {
        return Err(recovery_evidence_error());
    }
    let target = fs::read_link(path).map_err(|_| recovery_evidence_error())?;
    let expected_target = match &mutation.expected {
        LinkState::ManagedSymlink { target }
        | LinkState::BrokenSymlink { target }
        | LinkState::UnknownSymlink { target } => Some(target),
        LinkState::Missing | LinkState::Directory { .. } => None,
    };
    if mutation.desired_target.as_ref() != Some(&target) && expected_target != Some(&target) {
        return Err(recovery_evidence_error());
    }
    Ok(())
}

fn cleanup_redundant_link_backup(mutation: &LinkMutation) -> Result<(), SkillError> {
    let (LinkState::Directory { tree_hash }, Some(backup)) = (&mutation.expected, &mutation.backup)
    else {
        return Ok(());
    };
    match optional_directory_hash_recovery(backup)? {
        None => Ok(()),
        Some(observed) if observed == *tree_hash => remove_safe_entry_and_sync(backup),
        Some(_) => Err(recovery_evidence_error()),
    }
}

fn remove_current_link_if_present(path: &Path) -> Result<(), SkillError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(_) => Err(recovery_evidence_error()),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::remove_file(path).map_err(|_| recovery_evidence_error())?;
            sync_directory(path.parent().ok_or_else(recovery_evidence_error)?)
        }
        Ok(_) => Err(recovery_evidence_error()),
    }
}

fn create_symlink_atomic(
    spec: &TransactionSpec,
    mutation: &LinkMutation,
    index: usize,
    target: &Path,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    let temporary = link_temp_path(spec, mutation, index);
    validate_link_runtime_bounds(mutation, paths)?;
    validate_link_path(&temporary, &verified_catalog_targets(paths)?)?;
    cleanup_link_temporary(&temporary, mutation)?;
    let parent = mutation.path.parent().ok_or_else(recovery_evidence_error)?;
    create_symlink(target, &temporary)?;
    sync_directory(parent)?;
    validate_link_runtime_bounds(mutation, paths)?;
    validate_link_path(&temporary, &verified_catalog_targets(paths)?)?;
    if fs::read_link(&temporary).map_err(|_| recovery_evidence_error())? != target
        || !matches!(
            fs::symlink_metadata(&mutation.path),
            Err(error) if error.kind() == ErrorKind::NotFound
        )
    {
        return Err(recovery_evidence_error());
    }
    fs::rename(&temporary, &mutation.path).map_err(|_| recovery_evidence_error())?;
    sync_directory(parent)
}

fn finish_successful_transaction(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_committed_state(spec, paths)?;
    remove_staging_operation(paths, &spec.operation_id)?;
    cleanup_obsolete_backups(spec, paths)?;
    remove_journal(paths, &spec.operation_id)
}

fn validate_committed_state(spec: &TransactionSpec, paths: &SkillsPaths) -> Result<(), SkillError> {
    validate_settings_precondition(&spec.settings_after)?;
    for mutation in &spec.directory_mutations {
        let desired = directory_after_hash(spec, mutation)?;
        if optional_directory_hash_recovery(&mutation.destination)? != desired {
            return Err(recovery_evidence_error());
        }
    }
    for mutation in &spec.link_mutations {
        let desired = match &mutation.desired_target {
            Some(target) => LinkState::ManagedSymlink {
                target: target.clone(),
            },
            None => LinkState::Missing,
        };
        if !link_matches_state(&mutation.path, &desired, paths)? {
            return Err(recovery_evidence_error());
        }
    }
    Ok(())
}

fn cleanup_obsolete_backups(spec: &TransactionSpec, paths: &SkillsPaths) -> Result<(), SkillError> {
    let retained = retained_import_backups(&spec.settings_after, paths);
    for mutation in &spec.directory_mutations {
        if !retained.contains(&lexical_absolute(&mutation.backup)?) {
            verify_and_remove_backup(&mutation.backup, mutation.expected_before_hash.as_deref())?;
        }
    }
    for mutation in &spec.link_mutations {
        let Some(backup) = &mutation.backup else {
            continue;
        };
        if retained.contains(&lexical_absolute(backup)?) {
            continue;
        }
        let expected_hash = match &mutation.expected {
            LinkState::Directory { tree_hash } => Some(tree_hash.as_str()),
            _ => None,
        };
        verify_and_remove_backup(backup, expected_hash)?;
    }
    Ok(())
}

fn retained_import_backups(
    snapshot: &SkillSettingsSnapshot,
    paths: &SkillsPaths,
) -> BTreeSet<PathBuf> {
    snapshot
        .managed_skills
        .iter()
        .flat_map(|records| records.values())
        .filter_map(|record| match &record.source {
            SkillSource::Imported { backup_path, .. } => paths.expand_user(backup_path),
            _ => None,
        })
        .filter_map(|path| lexical_absolute(&path).ok())
        .collect()
}

fn verify_and_remove_backup(path: &Path, expected_hash: Option<&str>) -> Result<(), SkillError> {
    let observed = optional_directory_hash_recovery(path)?;
    match (observed, expected_hash) {
        (None, _) => Ok(()),
        (Some(observed), Some(expected)) if observed == expected => {
            remove_safe_entry_and_sync(path)
        }
        (Some(_), None) => Err(recovery_evidence_error()),
        (Some(_), Some(_)) => Err(recovery_evidence_error()),
    }
}

fn remove_staging_operation(paths: &SkillsPaths, operation_id: &str) -> Result<(), SkillError> {
    validate_operation_id(operation_id)?;
    let operation = paths.staging_skills_dir().join(operation_id);
    match fs::symlink_metadata(&operation) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(&operation, error)),
        Ok(metadata) if metadata.file_type().is_dir() => {
            fs::remove_dir_all(&operation).map_err(|error| io_error(&operation, error))?;
            sync_directory(&paths.staging_skills_dir())
        }
        Ok(_) => Err(SkillError::RecoveryRequired {
            message: "a Skills staging operation requires manual recovery".into(),
        }),
    }
}

fn remove_journal(paths: &SkillsPaths, operation_id: &str) -> Result<(), SkillError> {
    let path = journal_path(paths, operation_id)?;
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(&path, error)),
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(SkillError::RecoveryRequired {
                message: "a Skills journal requires manual recovery".into(),
            });
        }
        Ok(_) => {}
    }
    fs::remove_file(&path).map_err(|error| io_error(&path, error))?;
    let root = paths.journals_skills_dir();
    sync_directory(&root)?;
    match fs::remove_dir(&root) {
        Ok(()) => {
            if let Some(parent) = root.parent() {
                sync_directory(parent)?;
            }
        }
        Err(error) if error.kind() == ErrorKind::DirectoryNotEmpty => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&root, error)),
    }
    Ok(())
}

pub fn recover_pending() -> Result<(), SkillError> {
    let paths = SkillsPaths::from_env()?;
    recover_pending_with_paths(&paths)
}

#[doc(hidden)]
pub fn recover_pending_with_paths(paths: &SkillsPaths) -> Result<(), SkillError> {
    validate_transaction_roots(paths, true).map_err(recovery_error)?;
    let _lock = acquire_skills_lock(paths).map_err(recovery_error)?;
    validate_transaction_roots(paths, true).map_err(recovery_error)?;
    recover_pending_locked(paths).map_err(recovery_error)
}

fn recover_pending_locked(paths: &SkillsPaths) -> Result<(), SkillError> {
    let journals = load_and_validate_all_journals(paths)?;
    let mut pending = Vec::with_capacity(journals.len());
    for journal in &journals {
        validate_transaction_roots(paths, false)?;
        validate_transaction_bounds(&journal.spec, paths)?;
        let disposition = if validate_rollback_evidence(&journal.spec, paths).is_ok() {
            RecoveryDisposition::RollBack
        } else {
            validate_committed_cleanup_evidence(&journal.spec, paths)?;
            RecoveryDisposition::FinishCommit
        };
        pending.push(disposition);
    }
    for (journal, disposition) in journals.into_iter().zip(pending) {
        validate_transaction_roots(paths, false)?;
        validate_transaction_bounds(&journal.spec, paths)?;
        match disposition {
            RecoveryDisposition::RollBack => {
                rollback_transaction(&journal.spec, paths)?;
                remove_staging_operation(paths, &journal.spec.operation_id)?;
                remove_journal(paths, &journal.spec.operation_id)?;
            }
            RecoveryDisposition::FinishCommit => {
                validate_committed_cleanup_evidence(&journal.spec, paths)?;
                finish_successful_transaction(&journal.spec, paths)?;
            }
        }
    }
    cleanup_abandoned_staging(paths, Utc::now())?;
    Ok(())
}

fn validate_committed_cleanup_evidence(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_transaction_roots(paths, false)?;
    validate_transaction_bounds(spec, paths)?;
    validate_committed_state(spec, paths)?;
    let staging = paths.staging_skills_dir().join(&spec.operation_id);
    if !matches!(
        fs::symlink_metadata(&staging),
        Err(error) if error.kind() == ErrorKind::NotFound
    ) {
        return Err(recovery_evidence_error());
    }
    for (index, mutation) in spec.directory_mutations.iter().enumerate() {
        if optional_directory_hash_recovery(&directory_temp_path(spec, paths, index)?)?.is_some() {
            return Err(recovery_evidence_error());
        }
        validate_cleanup_backup_evidence(
            &mutation.backup,
            mutation.expected_before_hash.as_deref(),
            spec,
            paths,
        )?;
    }
    for (index, mutation) in spec.link_mutations.iter().enumerate() {
        let temporary = link_temp_path(spec, mutation, index);
        if !matches!(
            fs::symlink_metadata(&temporary),
            Err(error) if error.kind() == ErrorKind::NotFound
        ) {
            return Err(recovery_evidence_error());
        }
        if let Some(backup) = &mutation.backup {
            let expected = match &mutation.expected {
                LinkState::Directory { tree_hash } => Some(tree_hash.as_str()),
                _ => return Err(recovery_evidence_error()),
            };
            validate_cleanup_backup_evidence(backup, expected, spec, paths)?;
        }
    }
    Ok(())
}

fn validate_cleanup_backup_evidence(
    backup: &Path,
    expected_hash: Option<&str>,
    spec: &TransactionSpec,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    let retained =
        retained_import_backups(&spec.settings_after, paths).contains(&lexical_absolute(backup)?);
    match (
        optional_directory_hash_recovery(backup)?,
        expected_hash,
        retained,
    ) {
        (None, _, false) => Ok(()),
        (Some(observed), Some(expected), _) if observed == expected => Ok(()),
        _ => Err(recovery_evidence_error()),
    }
}

fn recovery_error(_error: SkillError) -> SkillError {
    SkillError::RecoveryRequired {
        message: "pending Skills filesystem recovery requires manual attention".into(),
    }
}

fn load_and_validate_all_journals(paths: &SkillsPaths) -> Result<Vec<Journal>, SkillError> {
    let root = paths.journals_skills_dir();
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(&root, error)),
    };
    let mut paths_and_ids = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| io_error(&root, error))?;
        let file_name = entry
            .file_name()
            .into_string()
            .map_err(|_| recovery_evidence_error())?;
        let operation_id = file_name
            .strip_suffix(".json")
            .ok_or_else(recovery_evidence_error)?;
        validate_operation_id(operation_id).map_err(|_| recovery_evidence_error())?;
        paths_and_ids.push((operation_id.to_owned(), entry.path()));
    }
    paths_and_ids.sort_by(|left, right| left.0.cmp(&right.0));

    let mut journals = Vec::with_capacity(paths_and_ids.len());
    for (operation_id, path) in paths_and_ids {
        let journal = read_journal(&path)?;
        if journal.spec.operation_id != operation_id {
            return Err(recovery_evidence_error());
        }
        validate_transaction_bounds(&journal.spec, paths)?;
        journals.push(journal);
    }
    Ok(journals)
}

#[doc(hidden)]
pub fn crash_transaction_at_phase_for_test(
    spec: TransactionSpec,
    phase: JournalPhase,
) -> Result<(), SkillError> {
    let paths = SkillsPaths::from_env()?;
    let _lock = acquire_skills_lock(&paths)?;
    if has_pending_recovery_with_paths(&paths)? {
        return Err(SkillError::RecoveryRequired {
            message: "a pending Skills operation must be recovered first".into(),
        });
    }
    validate_transaction_bounds(&spec, &paths)?;
    validate_all_preconditions(&spec, &paths)?;
    write_journal(&paths, &spec, JournalPhase::Prepared)?;
    if phase == JournalPhase::Prepared {
        return Ok(());
    }
    match spec.order {
        TransactionOrder::ContentThenLinks => {
            apply_directories(&spec, &paths)?;
            write_journal(&paths, &spec, JournalPhase::ContentSwapped)?;
            if phase == JournalPhase::ContentSwapped {
                return Ok(());
            }
            apply_links(&spec, &paths, None)?;
            write_journal(&paths, &spec, JournalPhase::LinksSwapped)?;
            if phase == JournalPhase::LinksSwapped {
                return Ok(());
            }
        }
        TransactionOrder::LinksThenContent => {
            apply_links(&spec, &paths, None)?;
            write_journal(&paths, &spec, JournalPhase::LinksSwapped)?;
            if phase == JournalPhase::LinksSwapped {
                return Ok(());
            }
            apply_directories(&spec, &paths)?;
            write_journal(&paths, &spec, JournalPhase::ContentSwapped)?;
            if phase == JournalPhase::ContentSwapped {
                return Ok(());
            }
        }
    }
    write_skill_settings(&paths, &spec.settings_before, &spec.settings_after)?;
    write_journal(&paths, &spec, JournalPhase::SettingsWritten)
}

#[doc(hidden)]
pub fn crash_transaction_before_phase_for_test(
    spec: TransactionSpec,
    point: CrashPoint,
) -> Result<(), SkillError> {
    let paths = SkillsPaths::from_env()?;
    let _lock = acquire_skills_lock(&paths)?;
    if has_pending_recovery_with_paths(&paths)? {
        return Err(SkillError::RecoveryRequired {
            message: "a pending Skills operation must be recovered first".into(),
        });
    }
    validate_transaction_bounds(&spec, &paths)?;
    validate_all_preconditions(&spec, &paths)?;
    write_journal(&paths, &spec, JournalPhase::Prepared)?;

    match spec.order {
        TransactionOrder::ContentThenLinks => {
            apply_directories(&spec, &paths)?;
            if point == CrashPoint::AfterContentBeforePhase {
                return Ok(());
            }
            write_journal(&paths, &spec, JournalPhase::ContentSwapped)?;
            apply_links(&spec, &paths, None)?;
            if point == CrashPoint::AfterLinksBeforePhase {
                return Ok(());
            }
            write_journal(&paths, &spec, JournalPhase::LinksSwapped)?;
        }
        TransactionOrder::LinksThenContent => {
            apply_links(&spec, &paths, None)?;
            if point == CrashPoint::AfterLinksBeforePhase {
                return Ok(());
            }
            write_journal(&paths, &spec, JournalPhase::LinksSwapped)?;
            apply_directories(&spec, &paths)?;
            if point == CrashPoint::AfterContentBeforePhase {
                return Ok(());
            }
            write_journal(&paths, &spec, JournalPhase::ContentSwapped)?;
        }
    }
    write_skill_settings(&paths, &spec.settings_before, &spec.settings_after)?;
    debug_assert_eq!(point, CrashPoint::AfterSettingsBeforePhase);
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingMetadata {
    operation_id: String,
    created_at: String,
}

fn cleanup_abandoned_staging(paths: &SkillsPaths, now: DateTime<Utc>) -> Result<(), SkillError> {
    let root = paths.staging_skills_dir();
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(&root, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| io_error(&root, error))?;
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if !metadata.file_type().is_dir() || !private_directory(&metadata) {
            continue;
        }
        let Some(operation_id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if validate_operation_id(&operation_id).is_err() {
            continue;
        }
        let journal = journal_path(paths, &operation_id)?;
        if !matches!(
            fs::symlink_metadata(&journal),
            Err(error) if error.kind() == ErrorKind::NotFound
        ) {
            continue;
        }
        let metadata_path = path.join(STAGING_METADATA_FILE);
        let Some(staging) = read_staging_metadata(&metadata_path)? else {
            continue;
        };
        if staging.operation_id != operation_id {
            continue;
        }
        let Ok(created_at) = DateTime::parse_from_rfc3339(&staging.created_at) else {
            continue;
        };
        let created_at = created_at.with_timezone(&Utc);
        if now.signed_duration_since(created_at) <= chrono::Duration::hours(STALE_STAGING_AGE_HOURS)
        {
            continue;
        }
        fs::remove_dir_all(&path).map_err(|error| io_error(&path, error))?;
        sync_directory(&root)?;
    }
    Ok(())
}

#[cfg(unix)]
fn private_directory(metadata: &fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o077 == 0
}

#[cfg(not(unix))]
fn private_directory(_metadata: &fs::Metadata) -> bool {
    true
}

fn read_staging_metadata(path: &Path) -> Result<Option<StagingMetadata>, SkillError> {
    let mut file = match open_read_nofollow(path) {
        Ok(file) => file,
        Err(_) => return Ok(None),
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if !metadata.file_type().is_file()
        || metadata.len() > STAGING_METADATA_BYTES
        || !private_file(&metadata)
    {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    if Read::by_ref(&mut file)
        .take(STAGING_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > STAGING_METADATA_BYTES
    {
        return Ok(None);
    }
    Ok(serde_json::from_slice(&bytes).ok())
}

#[cfg(unix)]
fn private_file(metadata: &fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o077 == 0 && metadata.nlink() == 1
}

#[cfg(not(unix))]
fn private_file(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{SkillSettingsSnapshot, SkillsPaths, TransactionOrder, TransactionSpec};
    use crate::testenv::TestHome;
    use std::fs;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    fn empty_spec(operation_id: &str) -> TransactionSpec {
        let settings = SkillSettingsSnapshot {
            managed_skills: None,
            skill_assignments: None,
            skill_update_checked_at: None,
        };
        TransactionSpec {
            operation_id: operation_id.into(),
            order: TransactionOrder::ContentThenLinks,
            directory_mutations: Vec::new(),
            link_mutations: Vec::new(),
            settings_before: settings.clone(),
            settings_after: settings,
        }
    }

    #[test]
    fn operation_ids_require_canonical_hyphenated_uuid_text() {
        let valid = "10000000-0000-4000-8000-000000000006";
        assert_eq!(validate_operation_id(valid).unwrap().to_string(), valid);
        for invalid in [
            "../../escape",
            "10000000000040008000000000000006",
            "10000000-0000-4000-8000-000000000006/child",
            "10000000-0000-4000-8000-000000000006.JSON",
            "10000000-0000-4000-8000-00000000000A",
        ] {
            assert!(
                validate_operation_id(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn advisory_lock_is_private_conflicts_without_a_path_and_releases_on_drop() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let _home = TestHome::new("tx-lock");
        let paths = SkillsPaths::from_env().unwrap();
        let first = acquire_skills_lock(&paths).unwrap();
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(paths.skills_lock())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let conflict =
            acquire_skills_lock_with_timeout(&paths, Duration::from_millis(40)).unwrap_err();
        let crate::skills::SkillError::Conflict { path, .. } = conflict else {
            panic!("lock contention did not return Conflict");
        };
        assert!(path.is_empty());
        drop(first);
        acquire_skills_lock_with_timeout(&paths, Duration::from_millis(40)).unwrap();
    }

    #[test]
    fn journal_replacement_is_atomic_at_both_failure_seams() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let _home = TestHome::new("tx-journal");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "20000000-0000-4000-8000-000000000006";
        let spec = empty_spec(id);
        write_journal(&paths, &spec, JournalPhase::Prepared).unwrap();
        let path = journal_path(&paths, id).unwrap();
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let error = write_journal_with_failpoint(
            &paths,
            &spec,
            JournalPhase::ContentSwapped,
            Some(JournalWriteFailpoint::BeforeRename),
        )
        .unwrap_err();
        assert!(matches!(error, crate::skills::SkillError::Io { .. }));
        assert_eq!(read_journal(&path).unwrap().phase, JournalPhase::Prepared);

        let error = write_journal_with_failpoint(
            &paths,
            &spec,
            JournalPhase::ContentSwapped,
            Some(JournalWriteFailpoint::AfterRenameBeforeParentSync),
        )
        .unwrap_err();
        assert!(matches!(error, crate::skills::SkillError::Io { .. }));
        assert_eq!(
            read_journal(&path).unwrap().phase,
            JournalPhase::ContentSwapped
        );
        assert_eq!(
            fs::read_dir(paths.journals_skills_dir()).unwrap().count(),
            1,
            "atomic write seams must not retain temporary journal files"
        );
    }

    #[cfg(unix)]
    #[test]
    fn advisory_lock_times_out_cross_process_and_is_released_when_holder_is_killed() {
        let home = TestHome::new("tx-crash-lock");
        let paths = SkillsPaths::from_env().unwrap();
        let ready = home.home.join("lock-ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "skills::transaction::tests::advisory_lock_child_helper",
            ])
            .env("MUX_TRANSACTION_LOCK_CHILD_READY", &ready)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        while !ready.exists() && started.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            ready.exists(),
            "child process did not acquire the Skills lock"
        );
        let conflict =
            acquire_skills_lock_with_timeout(&paths, Duration::from_millis(75)).unwrap_err();
        assert!(
            matches!(conflict, crate::skills::SkillError::Conflict { ref path, .. } if path.is_empty())
        );

        child.kill().unwrap();
        child.wait().unwrap();
        acquire_skills_lock_with_timeout(&paths, Duration::from_secs(1)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    #[ignore]
    fn advisory_lock_child_helper() {
        let Some(ready) = std::env::var_os("MUX_TRANSACTION_LOCK_CHILD_READY") else {
            return;
        };
        let paths = SkillsPaths::from_env().unwrap();
        let _lock = acquire_skills_lock(&paths).unwrap();
        fs::write(ready, b"ready").unwrap();
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    #[test]
    fn rollback_failure_takes_precedence_and_returns_path_free_recovery_required() {
        let home = TestHome::new("tx-rollback-failure");
        let paths = SkillsPaths::from_env().unwrap();
        let mut spec = empty_spec("30000000-0000-4000-8000-000000000006");
        spec.settings_before.skill_update_checked_at = Some("before".into());
        spec.settings_after.skill_update_checked_at = Some("after".into());
        let error = finish_failed_transaction(
            &spec,
            &paths,
            crate::skills::SkillError::Io {
                message: "primary".into(),
                path: Some(home.home.display().to_string()),
            },
        )
        .unwrap_err();
        let crate::skills::SkillError::RecoveryRequired { message } = error else {
            panic!("rollback failure did not take precedence");
        };
        assert!(!message.contains(home.home.to_string_lossy().as_ref()));
    }

    #[cfg(unix)]
    #[test]
    fn stale_staging_cleanup_requires_old_matching_private_metadata_and_never_follows_links() {
        use std::os::unix::fs::{symlink, OpenOptionsExt};

        let home = TestHome::new("tx-stale-staging");
        let paths = SkillsPaths::from_env().unwrap();
        let now = DateTime::parse_from_rfc3339("2026-07-17T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let stale = "40000000-0000-4000-8000-000000000006";
        let exact_day = "40000000-0000-4000-8000-000000000007";
        let malformed = "40000000-0000-4000-8000-000000000008";
        let wrong_id = "40000000-0000-4000-8000-000000000009";
        let linked_root = "40000000-0000-4000-8000-00000000000a";
        let linked_metadata = "40000000-0000-4000-8000-00000000000b";
        let journaled = "40000000-0000-4000-8000-00000000000c";

        write_staging_case(
            &paths,
            stale,
            format!(r#"{{"operation_id":"{stale}","created_at":"2026-07-16T11:59:59Z"}}"#),
        );
        write_staging_case(
            &paths,
            exact_day,
            format!(r#"{{"operation_id":"{exact_day}","created_at":"2026-07-16T12:00:00Z"}}"#),
        );
        write_staging_case(&paths, malformed, b"not-json");
        write_staging_case(
            &paths,
            wrong_id,
            r#"{"operation_id":"40000000-0000-4000-8000-0000000000ff","created_at":"2026-07-15T00:00:00Z"}"#,
        );

        let outside = home.home.join("outside-staging");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"untouched").unwrap();
        symlink(&outside, paths.staging_skills_dir().join(linked_root)).unwrap();

        let linked_metadata_root = write_staging_case(
            &paths,
            linked_metadata,
            format!(
                r#"{{"operation_id":"{linked_metadata}","created_at":"2026-07-15T00:00:00Z"}}"#
            ),
        );
        fs::remove_file(linked_metadata_root.join(STAGING_METADATA_FILE)).unwrap();
        let external_metadata = home.home.join("external-metadata");
        fs::write(&external_metadata, b"{}").unwrap();
        symlink(
            &external_metadata,
            linked_metadata_root.join(STAGING_METADATA_FILE),
        )
        .unwrap();

        write_staging_case(
            &paths,
            journaled,
            format!(r#"{{"operation_id":"{journaled}","created_at":"2026-07-15T00:00:00Z"}}"#),
        );
        let journal_path = paths
            .journals_skills_dir()
            .join(format!("{journaled}.json"));
        let journal = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(journal_path)
            .unwrap();
        journal.sync_all().unwrap();

        cleanup_abandoned_staging(&paths, now).unwrap();

        assert!(!paths.staging_skills_dir().join(stale).exists());
        for retained in [
            exact_day,
            malformed,
            wrong_id,
            linked_root,
            linked_metadata,
            journaled,
        ] {
            assert!(
                fs::symlink_metadata(paths.staging_skills_dir().join(retained)).is_ok(),
                "cleanup removed retained staging case {retained}"
            );
        }
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"untouched");
    }

    #[cfg(unix)]
    fn write_staging_case(
        paths: &SkillsPaths,
        operation_id: &str,
        metadata: impl AsRef<[u8]>,
    ) -> PathBuf {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

        let path = paths.staging_skills_dir().join(operation_id);
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(&path).unwrap();
        let metadata_path = path.join(STAGING_METADATA_FILE);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(metadata_path)
            .unwrap();
        file.write_all(metadata.as_ref()).unwrap();
        file.sync_all().unwrap();
        path
    }
}
