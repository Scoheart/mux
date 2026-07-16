#[cfg(unix)]
use super::anchored::{AnchoredFileKind, AnchoredRoot};
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
const MAX_PENDING_JOURNAL_FILES: u64 = 128;
const MAX_PENDING_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
const JOURNAL_SCHEMA_VERSION: u32 = 1;
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
    #[serde(default = "journal_schema_version")]
    version: u32,
    spec: TransactionSpec,
    phase: JournalPhase,
}

fn journal_schema_version() -> u32 {
    JOURNAL_SCHEMA_VERSION
}

#[derive(Debug)]
struct LoadedJournals {
    journals: Vec<Journal>,
    temp_promotions: Vec<JournalTempPromotion>,
    retired_operation_ids: Vec<String>,
}

#[derive(Debug)]
struct JournalTempPromotion {
    temporary: PathBuf,
    destination: PathBuf,
    journal: Journal,
}

#[derive(Debug, Default)]
struct JournalFileSet {
    active: Option<PathBuf>,
    temporary: Option<PathBuf>,
    retiring: Option<PathBuf>,
    retired: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JournalWriteFailpoint {
    BeforeRename,
    AfterRenameBeforeParentSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JournalRetireFailpoint {
    RenameToRetiringBeforeSync,
    RetiringSynced,
    RenameToRetiredBeforeSync,
    RetiredSynced,
    RetiredUnlinkedBeforeSync,
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
    let file = open_private_lock(paths)?;
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
fn open_private_lock(paths: &SkillsPaths) -> Result<File, SkillError> {
    use rustix::fs::{openat, Mode, OFlags};

    let path = paths.skills_lock();
    let mux = AnchoredRoot::open(paths.mux_dir())?;
    let directory = mux.root_directory()?;
    let descriptor = openat(
        &directory,
        "skills.lock",
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::from(0o600),
    )
    .map_err(|error| io_error(&path, error.into()))?;
    let file = File::from(descriptor);
    let metadata = file.metadata().map_err(|error| io_error(&path, error))?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(SkillError::UnsafePath {
            message: "the Skills operation lock is not a private regular file".into(),
            path: String::new(),
        });
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| io_error(&path, error))?;
    directory
        .sync_all()
        .map_err(|error| io_error(paths.mux_dir(), error))?;
    Ok(file)
}

#[cfg(not(unix))]
fn open_private_lock(paths: &SkillsPaths) -> Result<File, SkillError> {
    let path = paths.skills_lock();
    let metadata = fs::symlink_metadata(&path).ok();
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
        .open(&path)
        .map_err(|error| io_error(&path, error))
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

fn journal_retiring_path(paths: &SkillsPaths, operation_id: &str) -> Result<PathBuf, SkillError> {
    validate_operation_id(operation_id)?;
    Ok(paths
        .journals_skills_dir()
        .join(format!("{operation_id}.retiring")))
}

fn journal_retired_path(paths: &SkillsPaths, operation_id: &str) -> Result<PathBuf, SkillError> {
    validate_operation_id(operation_id)?;
    Ok(paths
        .journals_skills_dir()
        .join(format!("{operation_id}.retired")))
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
    if journal_entry_exists(&journal_retiring_path(paths, &spec.operation_id)?)?
        || journal_entry_exists(&journal_retired_path(paths, &spec.operation_id)?)?
    {
        return Err(SkillError::RecoveryRequired {
            message: "the Skills transaction journal requires recovery".into(),
        });
    }
    let destination = journal_path(paths, &spec.operation_id)?;
    let temporary = journal_temp_path(paths, &spec.operation_id)?;
    remove_abandoned_journal_temp(&temporary)?;
    let bytes = serde_json::to_vec(&Journal {
        version: JOURNAL_SCHEMA_VERSION,
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
    AnchoredRoot::open_or_create_private_absolute(&root)?;
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
    let journal: Journal =
        serde_json::from_slice(&bytes).map_err(|_| SkillError::RecoveryRequired {
            message: "a Skills journal is malformed".into(),
        })?;
    if journal.version != JOURNAL_SCHEMA_VERSION {
        return Err(SkillError::RecoveryRequired {
            message: "a Skills journal uses an unsupported schema version".into(),
        });
    }
    Ok(journal)
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
    let loaded = load_and_validate_all_journals(paths)?;
    Ok(!loaded.journals.is_empty() || !loaded.temp_promotions.is_empty())
}

pub fn execute_transaction(spec: TransactionSpec) -> Result<(), SkillError> {
    execute_transaction_with_failpoint(spec, None)
}

#[doc(hidden)]
pub fn execute_transaction_with_failpoint(
    spec: TransactionSpec,
    failpoint: Option<Failpoint>,
) -> Result<(), SkillError> {
    let paths = SkillsPaths::resolve_from_env()?;
    paths.ensure_mux_root()?;
    let _lock = acquire_skills_lock(&paths)?;
    paths.ensure_transaction_roots()?;
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
    let retained_imports = retained_import_backups(&spec.settings_after, paths);
    let mut mutation_paths = BTreeSet::new();
    let mut physical_link_paths = BTreeSet::new();
    let mut backup_paths = BTreeSet::new();

    for mutation in &spec.directory_mutations {
        validate_central_destination(&mutation.destination, paths)?;
        validate_skill_name_component(&mutation.destination)?;
        insert_unique(&mut mutation_paths, &mutation.destination)?;
        validate_backup_path(&mutation.backup, paths)?;
        insert_unique(&mut backup_paths, &mutation.backup)?;
        if (mutation.retain_backup
            || retained_imports.contains(&lexical_absolute(&mutation.backup)?))
            && mutation.expected_before_hash.is_none()
        {
            return Err(SkillError::InvalidSource {
                message: "a retained Skill backup requires a reviewed before hash".into(),
            });
        }
        if let Some(replacement) = &mutation.replacement {
            validate_staging_path(replacement, &spec.operation_id, paths)?;
        }
    }
    for mutation in &spec.link_mutations {
        let physical_parent = validate_link_path(&mutation.path, &catalog_targets)?;
        validate_skill_name_component(&mutation.path)?;
        insert_unique(&mut mutation_paths, &mutation.path)?;
        let name = mutation
            .path
            .file_name()
            .ok_or_else(unsafe_transaction_path)?
            .to_os_string();
        if !physical_link_paths.insert((physical_parent_key(&physical_parent)?, name)) {
            return Err(SkillError::InvalidSource {
                message: "a Skills transaction contains physically duplicate link targets".into(),
            });
        }
        if let Some(target) = &mutation.desired_target {
            validate_central_destination(target, paths)?;
            validate_skill_name_component(target)?;
        }
        validate_managed_link_spec(mutation, &spec.settings_before, paths)?;
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
    validate_backup_paths_are_disjoint(&backup_paths, paths)?;
    Ok(())
}

fn validate_backup_paths_are_disjoint(
    backups: &BTreeSet<PathBuf>,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    for backup in backups {
        if backup
            .ancestors()
            .skip(1)
            .any(|ancestor| backups.contains(ancestor))
        {
            return Err(SkillError::InvalidSource {
                message: "a Skills transaction contains overlapping backup paths".into(),
            });
        }
    }
    let mut physical = BTreeSet::new();
    for backup in backups {
        let canonical = canonicalize_deepest(backup)?;
        if !physical.insert(canonical) {
            return Err(overlapping_backups());
        }
    }
    for backup in &physical {
        if backup
            .ancestors()
            .skip(1)
            .any(|ancestor| physical.contains(ancestor))
        {
            return Err(overlapping_backups());
        }
        for control in [
            paths.skills_dir(),
            paths.staging_skills_dir(),
            paths.journals_skills_dir(),
            paths.skills_lock(),
        ] {
            let control = canonicalize_deepest(&control)?;
            if backup == &control || backup.starts_with(&control) || control.starts_with(backup) {
                return Err(overlapping_backups());
            }
        }
    }
    Ok(())
}

fn overlapping_backups() -> SkillError {
    SkillError::InvalidSource {
        message: "a Skills transaction contains overlapping backup paths".into(),
    }
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

fn validate_managed_link_spec(
    mutation: &LinkMutation,
    settings_before: &SkillSettingsSnapshot,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    let LinkState::ManagedSymlink { target } = &mutation.expected else {
        return Ok(());
    };
    let central = managed_link_central_target(&mutation.path, target, paths)?;
    let name = central
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(unsafe_transaction_path)?;
    if !settings_before
        .managed_skills
        .as_ref()
        .is_some_and(|records| records.contains_key(name))
    {
        return Err(SkillError::InvalidSource {
            message: "a reviewed managed link is not bound to managed Skills settings".into(),
        });
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedCatalogTarget {
    lexical: PathBuf,
    canonical: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PhysicalParentKey {
    Identity(u64, u64),
    Missing(PathBuf),
}

#[cfg(unix)]
fn physical_parent_key(path: &Path) -> Result<PhysicalParentKey, SkillError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            Ok(PhysicalParentKey::Identity(metadata.dev(), metadata.ino()))
        }
        Ok(_) => Err(unsafe_transaction_path()),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Ok(PhysicalParentKey::Missing(path.to_path_buf()))
        }
        Err(error) => Err(io_error(path, error)),
    }
}

#[cfg(not(unix))]
fn physical_parent_key(_path: &Path) -> Result<PhysicalParentKey, SkillError> {
    Err(SkillError::InvalidSource {
        message: "secure Skill transaction targets are unavailable on this platform".into(),
    })
}

fn verified_catalog_targets(paths: &SkillsPaths) -> Result<Vec<VerifiedCatalogTarget>, SkillError> {
    let mut targets = std::collections::BTreeMap::new();
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
    Ok(targets
        .into_iter()
        .map(|(lexical, canonical)| VerifiedCatalogTarget { lexical, canonical })
        .collect())
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
    targets: &mut std::collections::BTreeMap<PathBuf, PathBuf>,
    global_dir: &str,
) -> Result<(), SkillError> {
    let expanded = paths.expand_user(global_dir).ok_or_else(invalid_catalog)?;
    if !expanded.is_absolute() {
        return Err(invalid_catalog());
    }
    let lexical = lexical_absolute(&expanded)?;
    let canonical = canonicalize_deepest(&expanded)?;
    let canonical_home = fs::canonicalize(paths.user_home()).map_err(|_| invalid_catalog())?;
    if canonical == canonical_home || !canonical.starts_with(&canonical_home) {
        return Err(invalid_catalog());
    }
    if let Some(existing) = targets.get(&lexical) {
        if existing != &canonical {
            return Err(invalid_catalog());
        }
    } else {
        targets.insert(lexical, canonical);
    }
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

fn validate_link_path(
    path: &Path,
    targets: &[VerifiedCatalogTarget],
) -> Result<PathBuf, SkillError> {
    validate_no_traversal(path)?;
    if !path.is_absolute() || !path.file_name().is_some_and(valid_path_component) {
        return Err(unsafe_transaction_path());
    }
    let parent = path.parent().ok_or_else(unsafe_transaction_path)?;
    let lexical_parent = lexical_absolute(parent)?;
    let canonical_parent = canonicalize_deepest(parent)?;
    if !targets
        .iter()
        .any(|target| target.lexical == lexical_parent && target.canonical == canonical_parent)
    {
        return Err(SkillError::UnsafePath {
            message: "a Skill link is outside the currently verified Agent targets".into(),
            path: String::new(),
        });
    }
    Ok(canonical_parent)
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
        validate_managed_link_review_precondition(spec, paths, mutation)?;
        validate_link_precondition(paths, mutation)?;
    }
    Ok(())
}

fn validate_managed_link_review_precondition(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &LinkMutation,
) -> Result<(), SkillError> {
    let LinkState::ManagedSymlink { target } = &mutation.expected else {
        return Ok(());
    };
    let central = managed_link_central_target(&mutation.path, target, paths)?;
    let name = central
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(unsafe_transaction_path)?;
    let expected_hash = spec
        .settings_before
        .managed_skills
        .as_ref()
        .and_then(|records| records.get(name))
        .map(|record| record.content_hash.as_str())
        .ok_or_else(|| SkillError::InvalidSource {
            message: "a reviewed managed link is not bound to managed Skills settings".into(),
        })?;
    if optional_directory_hash(&central)?.as_deref() != Some(expected_hash) {
        return Err(stale("managed Skill content changed after link review"));
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
    apply_directory_with_hook(spec, paths, mutation, index, None)
}

fn apply_directory_with_hook(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &DirectoryMutation,
    index: usize,
    mut before_mutation: Option<&mut dyn FnMut()>,
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
    if let Some(hook) = before_mutation.take() {
        hook();
    }
    if mutation.expected_before_hash.is_some() {
        if let Err(error) = rename_noreplace(&mutation.destination, &mutation.backup) {
            remove_safe_entry_and_sync(&temporary)?;
            return if matches!(
                fs::symlink_metadata(&mutation.destination),
                Err(current) if current.kind() == ErrorKind::NotFound
            ) {
                Err(stale("central Skill content changed before backup"))
            } else {
                Err(error)
            };
        }
        let moved_hash = optional_directory_hash(&mutation.backup);
        let destination_missing = matches!(
            fs::symlink_metadata(&mutation.destination),
            Err(error) if error.kind() == ErrorKind::NotFound
        );
        if !destination_missing
            || !matches!(&moved_hash, Ok(hash) if hash == &mutation.expected_before_hash)
        {
            if rename_noreplace(&mutation.backup, &mutation.destination).is_err() {
                return Err(SkillError::RecoveryRequired {
                    message:
                        "an unreviewed central Skill tree was quarantined and could not be restored"
                            .into(),
                });
            }
            remove_safe_entry_and_sync(&temporary)?;
            return match moved_hash {
                Ok(_) => Err(stale("central Skill content changed before backup")),
                Err(_) => Err(stale("central Skill content changed type before backup")),
            };
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
        rename_noreplace(&temporary, &mutation.destination).map_err(|_| {
            SkillError::RecoveryRequired {
                message: "the central Skill replacement slot changed during commit".into(),
            }
        })?;
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
    AnchoredRoot::open_or_create_private_absolute(parent)?;
    verify_physical_root_membership(parent, root)?;
    Ok(())
}

fn rename_same_parent_noreplace(from: &Path, to: &Path) -> Result<(), SkillError> {
    let from_parent = from.parent().ok_or_else(unsafe_transaction_path)?;
    let to_parent = to.parent().ok_or_else(unsafe_transaction_path)?;
    if lexical_absolute(from_parent)? != lexical_absolute(to_parent)? {
        return Err(unsafe_transaction_path());
    }
    let from_name = from.file_name().ok_or_else(unsafe_transaction_path)?;
    let to_name = to.file_name().ok_or_else(unsafe_transaction_path)?;
    let parent = AnchoredRoot::open(from_parent)?;
    parent.rename_entry_noreplace(from_name, to_name, from)
}

fn rename_noreplace(from: &Path, to: &Path) -> Result<(), SkillError> {
    let from_parent = from.parent().ok_or_else(unsafe_transaction_path)?;
    let to_parent = to.parent().ok_or_else(unsafe_transaction_path)?;
    if lexical_absolute(from_parent)? == lexical_absolute(to_parent)? {
        return rename_same_parent_noreplace(from, to);
    }
    let source = AnchoredRoot::open(from_parent)?;
    let destination = AnchoredRoot::open(to_parent)?;
    source.rename_entry_noreplace_to(
        from.file_name().ok_or_else(unsafe_transaction_path)?,
        &destination,
        to.file_name().ok_or_else(unsafe_transaction_path)?,
        from,
    )
}

fn exchange_same_parent(left: &Path, right: &Path) -> Result<(), SkillError> {
    let left_parent = left.parent().ok_or_else(unsafe_transaction_path)?;
    let right_parent = right.parent().ok_or_else(unsafe_transaction_path)?;
    if lexical_absolute(left_parent)? != lexical_absolute(right_parent)? {
        return Err(unsafe_transaction_path());
    }
    let parent = AnchoredRoot::open(left_parent)?;
    parent.exchange_entries(
        left.file_name().ok_or_else(unsafe_transaction_path)?,
        right.file_name().ok_or_else(unsafe_transaction_path)?,
        left,
    )
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
    apply_link_with_hook(spec, paths, mutation, index, None)
}

fn apply_link_with_hook(
    spec: &TransactionSpec,
    paths: &SkillsPaths,
    mutation: &LinkMutation,
    index: usize,
    mut before_mutation: Option<&mut dyn FnMut()>,
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
        if let Some(hook) = before_mutation.take() {
            hook();
        }
        rename_noreplace(&mutation.path, backup)?;
        let expected_hash = match &mutation.expected {
            LinkState::Directory { tree_hash } => tree_hash,
            _ => unreachable!(),
        };
        let observed = optional_directory_hash(backup);
        if !matches!(&observed, Ok(Some(hash)) if hash == expected_hash) {
            if rename_noreplace(backup, &mutation.path).is_err() {
                return Err(SkillError::RecoveryRequired {
                    message: "an unreviewed Agent Skill directory was quarantined and could not be restored".into(),
                });
            }
            return Err(stale("an Agent Skill directory changed before backup"));
        }
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
            if let Some(hook) = before_mutation.take() {
                hook();
            }
            if matches!(
                mutation.expected,
                LinkState::Missing | LinkState::Directory { .. }
            ) {
                rename_same_parent_noreplace(&temporary, &mutation.path)?;
            } else {
                exchange_same_parent(&temporary, &mutation.path)?;
                let evidence =
                    link_entry_matches_state(&temporary, &mutation.path, &mutation.expected, paths);
                if !matches!(evidence, Ok(true)) {
                    if exchange_same_parent(&temporary, &mutation.path).is_err() {
                        return Err(SkillError::RecoveryRequired {
                            message: "a concurrently changed Skill link could not be restored after exchange".into(),
                        });
                    }
                    remove_safe_entry_and_sync(&temporary)?;
                    return match evidence {
                        Ok(false) => Err(stale("a reviewed Skill link changed before exchange")),
                        Err(error) => Err(error),
                        Ok(true) => unreachable!(),
                    };
                }
                remove_safe_entry_and_sync(&temporary)?;
            }
        }
        None if matches!(mutation.expected, LinkState::Directory { .. }) => {
            validate_link_runtime_bounds(mutation, paths)?;
            validate_link_directory_backup(mutation)?;
        }
        None => {
            validate_link_runtime_bounds(mutation, paths)?;
            validate_link_precondition(paths, mutation)?;
            if let Some(hook) = before_mutation.take() {
                hook();
            }
            match fs::symlink_metadata(&mutation.path) {
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    if !matches!(mutation.expected, LinkState::Missing) {
                        return Err(stale("a reviewed Skill link disappeared before removal"));
                    }
                }
                Err(error) => return Err(io_error(&mutation.path, error)),
                Ok(_) => {
                    if let Err(error) = rename_same_parent_noreplace(&mutation.path, &temporary) {
                        return if matches!(
                            fs::symlink_metadata(&mutation.path),
                            Err(current) if current.kind() == ErrorKind::NotFound
                        ) {
                            Err(stale("a reviewed Skill link changed before removal"))
                        } else {
                            Err(error)
                        };
                    }
                    let evidence = link_entry_matches_state(
                        &temporary,
                        &mutation.path,
                        &mutation.expected,
                        paths,
                    );
                    if !matches!(evidence, Ok(true)) {
                        if rename_same_parent_noreplace(&temporary, &mutation.path).is_err() {
                            return Err(SkillError::RecoveryRequired {
                                message: "a concurrently changed Skill entry was quarantined and could not be restored".into(),
                            });
                        }
                        return match evidence {
                            Ok(false) => Err(stale("a reviewed Skill link changed before removal")),
                            Err(error) => Err(error),
                            Ok(true) => unreachable!(),
                        };
                    }
                    fs::remove_file(&temporary).map_err(|error| io_error(&temporary, error))?;
                    sync_directory(parent)?;
                }
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
        managed_link_central_target(&mutation.path, target, paths)?;
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
    let lexical = lexical_absolute(parent)?;
    let expected = canonicalize_deepest(parent)?;
    if !targets
        .iter()
        .any(|target| target.lexical == lexical && target.canonical == expected)
    {
        return Err(unsafe_transaction_path());
    }
    AnchoredRoot::open_or_create_absolute(parent)?;
    let actual = canonicalize_deepest(parent)?;
    if actual != expected {
        return Err(unsafe_transaction_path());
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
    link_entry_matches_state(path, path, expected, paths)
}

fn link_entry_matches_state(
    entry_path: &Path,
    logical_path: &Path,
    expected: &LinkState,
    paths: &SkillsPaths,
) -> Result<bool, SkillError> {
    let metadata = match fs::symlink_metadata(entry_path) {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(matches!(expected, LinkState::Missing));
        }
        Err(error) => return Err(io_error(entry_path, error)),
        Ok(metadata) => metadata,
    };
    match expected {
        LinkState::Missing => Ok(false),
        LinkState::Directory { tree_hash } => {
            if !metadata.file_type().is_dir() {
                return Ok(false);
            }
            Ok(hash_tree(entry_path)? == *tree_hash)
        }
        LinkState::ManagedSymlink { target } => {
            if !metadata.file_type().is_symlink()
                || fs::read_link(entry_path).map_err(|error| io_error(entry_path, error))?
                    != *target
            {
                return Ok(false);
            }
            let central_target = managed_link_central_target(logical_path, target, paths)?;
            let target_canonical = match fs::canonicalize(&central_target) {
                Ok(path) => path,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(io_error(&central_target, error)),
            };
            let resolved = resolve_link_target(logical_path, target)?;
            let link_canonical = match fs::canonicalize(&resolved) {
                Ok(path) => path,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(io_error(&resolved, error)),
            };
            Ok(link_canonical == target_canonical)
        }
        LinkState::BrokenSymlink { target } => {
            if !metadata.file_type().is_symlink()
                || fs::read_link(entry_path).map_err(|error| io_error(entry_path, error))?
                    != *target
            {
                return Ok(false);
            }
            let resolved = resolve_link_target(logical_path, target)?;
            match fs::metadata(&resolved) {
                Ok(_) => Ok(false),
                Err(error)
                    if error.kind() == ErrorKind::NotFound || is_symlink_loop_error(&error) =>
                {
                    Ok(true)
                }
                Err(error) => Err(io_error(&resolved, error)),
            }
        }
        LinkState::UnknownSymlink { target } => {
            if !metadata.file_type().is_symlink()
                || fs::read_link(entry_path).map_err(|error| io_error(entry_path, error))?
                    != *target
            {
                return Ok(false);
            }
            let raw_resolved = resolve_link_target(logical_path, target)?;
            let resolved = match fs::canonicalize(&raw_resolved) {
                Ok(path) => path,
                Err(error)
                    if error.kind() == ErrorKind::NotFound || is_symlink_loop_error(&error) =>
                {
                    return Ok(false);
                }
                Err(error) => return Err(io_error(&raw_resolved, error)),
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
    let backup = optional_directory_hash_recovery(&mutation.backup);
    match &mutation.expected_before_hash {
        None => {
            if backup.as_ref().is_ok_and(Option::is_some)
                || (destination.is_some() && destination.as_ref() != after_hash.as_ref())
            {
                return Err(recovery_evidence_error());
            }
            if backup.is_err() {
                return Err(recovery_evidence_error());
            }
        }
        Some(before) if destination.as_deref() == Some(before.as_str()) => match &backup {
            Ok(None) => {}
            Ok(Some(hash)) if hash == before => {}
            _ => return Err(recovery_evidence_error()),
        },
        Some(before) => {
            if destination.is_some() && destination.as_ref() != after_hash.as_ref() {
                return Err(recovery_evidence_error());
            }
            if !matches!(&backup, Ok(Some(hash)) if hash == before) && destination.is_some() {
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
    let backup_hash = optional_directory_hash_recovery(&mutation.backup);
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
            if !matches!(backup_hash, Ok(None)) {
                return Err(recovery_evidence_error());
            }
        }
        Some(before_hash) => {
            if destination_hash.as_deref() == Some(before_hash.as_str()) {
                if let Some(observed_backup) = backup_hash? {
                    if observed_backup != *before_hash {
                        return Err(recovery_evidence_error());
                    }
                    validate_directory_recovery_bounds(spec, paths, mutation, index)?;
                    remove_safe_entry_and_sync(&mutation.backup)?;
                }
                return Ok(());
            }
            if !matches!(&backup_hash, Ok(Some(hash)) if hash == before_hash) {
                if destination_hash.is_none()
                    && journal_entry_exists(&mutation.backup).unwrap_or(false)
                {
                    rename_noreplace(&mutation.backup, &mutation.destination)
                        .map_err(|_| recovery_evidence_error())?;
                    return Err(recovery_evidence_error());
                }
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
            rename_noreplace(&mutation.backup, &mutation.destination)
                .map_err(|_| recovery_evidence_error())?;
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
    cleanup_link_temporary(&temporary, mutation, paths)?;
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
        rename_noreplace(backup, &mutation.path).map_err(|_| recovery_evidence_error())?;
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
            let central = managed_link_central_target(&mutation.path, target, paths)?;
            let metadata = fs::symlink_metadata(&central).map_err(|_| recovery_evidence_error())?;
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

fn managed_link_central_target(
    link: &Path,
    raw_target: &Path,
    paths: &SkillsPaths,
) -> Result<PathBuf, SkillError> {
    validate_skill_name_component(link)?;
    let name = link
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(unsafe_transaction_path)?;
    let central = paths.central_skill(name);
    validate_central_destination(&central, paths)?;
    validate_skill_name_component(&central)?;
    let resolved = resolve_link_target(link, raw_target)?;
    if canonicalize_deepest(&resolved)? != canonicalize_deepest(&central)? {
        return Err(SkillError::InvalidSource {
            message: "a reviewed managed link does not resolve to its exact central Skill".into(),
        });
    }
    Ok(central)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkTemporaryEvidence {
    Missing,
    ExpectedOrDesired,
    Opaque,
}

fn cleanup_link_temporary(
    path: &Path,
    mutation: &LinkMutation,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    match classify_link_temporary(path, mutation)? {
        LinkTemporaryEvidence::Missing => Ok(()),
        LinkTemporaryEvidence::ExpectedOrDesired => {
            remove_safe_entry_and_sync(path).map_err(|_| recovery_evidence_error())
        }
        LinkTemporaryEvidence::Opaque => {
            let original_missing = matches!(
                fs::symlink_metadata(&mutation.path),
                Err(error) if error.kind() == ErrorKind::NotFound
            );
            if original_missing {
                rename_same_parent_noreplace(path, &mutation.path)
                    .map_err(|_| recovery_evidence_error())?;
                return Err(recovery_evidence_error());
            }
            let original_is_desired = match &mutation.desired_target {
                Some(target) => link_matches_state(
                    &mutation.path,
                    &LinkState::ManagedSymlink {
                        target: target.clone(),
                    },
                    paths,
                )
                .unwrap_or(false),
                None => false,
            };
            if original_is_desired {
                exchange_same_parent(path, &mutation.path)
                    .map_err(|_| recovery_evidence_error())?;
                if remove_safe_entry_and_sync(path).is_err() {
                    return Err(recovery_evidence_error());
                }
            }
            Err(recovery_evidence_error())
        }
    }
}

fn validate_link_temporary_evidence(
    path: &Path,
    mutation: &LinkMutation,
) -> Result<(), SkillError> {
    classify_link_temporary(path, mutation).map(|_| ())
}

fn classify_link_temporary(
    path: &Path,
    mutation: &LinkMutation,
) -> Result<LinkTemporaryEvidence, SkillError> {
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(LinkTemporaryEvidence::Missing);
        }
        Err(_) => return Err(recovery_evidence_error()),
        Ok(metadata) => metadata,
    };
    if !metadata.file_type().is_symlink() {
        return Ok(LinkTemporaryEvidence::Opaque);
    }
    let target = fs::read_link(path).map_err(|_| recovery_evidence_error())?;
    let expected_target = match &mutation.expected {
        LinkState::ManagedSymlink { target }
        | LinkState::BrokenSymlink { target }
        | LinkState::UnknownSymlink { target } => Some(target),
        LinkState::Missing | LinkState::Directory { .. } => None,
    };
    if mutation.desired_target.as_ref() == Some(&target) || expected_target == Some(&target) {
        Ok(LinkTemporaryEvidence::ExpectedOrDesired)
    } else {
        Ok(LinkTemporaryEvidence::Opaque)
    }
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
    cleanup_link_temporary(&temporary, mutation, paths)?;
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
    rename_same_parent_noreplace(&temporary, &mutation.path)
        .map_err(|_| recovery_evidence_error())?;
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
    let retained_imports = retained_import_backups(&spec.settings_after, paths);

    // Validate every retained artifact before deleting any obsolete artifact. A
    // bad retained backup must leave the rest of the recovery evidence intact.
    for mutation in &spec.directory_mutations {
        if directory_backup_is_retained(mutation, &retained_imports)? {
            verify_retained_backup(
                &mutation.backup,
                mutation.expected_before_hash.as_deref(),
                paths,
            )?;
        }
    }
    for mutation in &spec.link_mutations {
        let Some(backup) = &mutation.backup else {
            continue;
        };
        let expected_hash = match &mutation.expected {
            LinkState::Directory { tree_hash } => Some(tree_hash.as_str()),
            _ => None,
        };
        if retained_imports.contains(&lexical_absolute(backup)?) {
            verify_retained_backup(backup, expected_hash, paths)?;
        }
    }

    for mutation in &spec.directory_mutations {
        if !directory_backup_is_retained(mutation, &retained_imports)? {
            verify_and_remove_backup(&mutation.backup, mutation.expected_before_hash.as_deref())?;
        }
    }
    for mutation in &spec.link_mutations {
        let Some(backup) = &mutation.backup else {
            continue;
        };
        if !retained_imports.contains(&lexical_absolute(backup)?) {
            let expected_hash = match &mutation.expected {
                LinkState::Directory { tree_hash } => Some(tree_hash.as_str()),
                _ => None,
            };
            verify_and_remove_backup(backup, expected_hash)?;
        }
    }
    Ok(())
}

fn directory_backup_is_retained(
    mutation: &DirectoryMutation,
    retained_imports: &BTreeSet<PathBuf>,
) -> Result<bool, SkillError> {
    Ok(mutation.retain_backup || retained_imports.contains(&lexical_absolute(&mutation.backup)?))
}

fn verify_retained_backup(
    path: &Path,
    expected_hash: Option<&str>,
    paths: &SkillsPaths,
) -> Result<(), SkillError> {
    validate_backup_path(path, paths)?;
    let expected_hash = expected_hash.ok_or_else(recovery_evidence_error)?;
    if optional_directory_hash_recovery(path)?.as_deref() != Some(expected_hash) {
        return Err(recovery_evidence_error());
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
    remove_journal_with_failpoint(paths, operation_id, None)
}

fn remove_journal_with_failpoint(
    paths: &SkillsPaths,
    operation_id: &str,
    failpoint: Option<JournalRetireFailpoint>,
) -> Result<(), SkillError> {
    let active = journal_path(paths, operation_id)?;
    let retiring = journal_retiring_path(paths, operation_id)?;
    let retired = journal_retired_path(paths, operation_id)?;
    let active_present = journal_entry_exists(&active)?;
    let retiring_present = journal_entry_exists(&retiring)?;
    let retired_present = journal_entry_exists(&retired)?;
    let present = [active_present, retiring_present, retired_present]
        .into_iter()
        .filter(|present| *present)
        .count();
    if present > 1 {
        return Err(recovery_evidence_error());
    }

    let root = paths.journals_skills_dir();
    if active_present {
        validate_retirement_journal(&active, operation_id)?;
        fs::rename(&active, &retiring).map_err(|error| io_error(&active, error))?;
        if failpoint == Some(JournalRetireFailpoint::RenameToRetiringBeforeSync) {
            return Err(recovery_evidence_error());
        }
        sync_directory(&root)?;
        if failpoint == Some(JournalRetireFailpoint::RetiringSynced) {
            return Err(recovery_evidence_error());
        }
    }
    if retiring_present || active_present {
        validate_retirement_journal(&retiring, operation_id)?;
        fs::rename(&retiring, &retired).map_err(|error| io_error(&retiring, error))?;
        if failpoint == Some(JournalRetireFailpoint::RenameToRetiredBeforeSync) {
            return Err(recovery_evidence_error());
        }
        sync_directory(&root)?;
        if failpoint == Some(JournalRetireFailpoint::RetiredSynced) {
            return Err(recovery_evidence_error());
        }
    }
    if retired_present || retiring_present || active_present {
        validate_retirement_journal(&retired, operation_id)?;
        fs::remove_file(&retired).map_err(|error| io_error(&retired, error))?;
        if failpoint != Some(JournalRetireFailpoint::RetiredUnlinkedBeforeSync) {
            // Failure here is benign: the directory already contains a
            // durably retired marker, so a crash can only resurrect that
            // inert marker and never an active transaction.
            let _ = sync_directory(&root);
        }
    }
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

fn validate_retirement_journal(path: &Path, operation_id: &str) -> Result<(), SkillError> {
    let journal = read_journal(path)?;
    if journal.spec.operation_id != operation_id {
        return Err(recovery_evidence_error());
    }
    Ok(())
}

fn journal_entry_exists(path: &Path) -> Result<bool, SkillError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

pub fn recover_pending() -> Result<(), SkillError> {
    let paths = SkillsPaths::resolve_from_env()?;
    paths.ensure_mux_root().map_err(recovery_error)?;
    let _lock = acquire_skills_lock(&paths).map_err(recovery_error)?;
    paths.ensure_transaction_roots().map_err(recovery_error)?;
    validate_transaction_roots(&paths, true).map_err(recovery_error)?;
    recover_pending_locked(&paths).map_err(recovery_error)
}

#[doc(hidden)]
pub fn recover_pending_with_paths(paths: &SkillsPaths) -> Result<(), SkillError> {
    validate_transaction_roots(paths, true).map_err(recovery_error)?;
    let _lock = acquire_skills_lock(paths).map_err(recovery_error)?;
    validate_transaction_roots(paths, true).map_err(recovery_error)?;
    recover_pending_locked(paths).map_err(recovery_error)
}

fn recover_pending_locked(paths: &SkillsPaths) -> Result<(), SkillError> {
    let loaded = load_and_validate_all_journals(paths)?;
    let mut pending = Vec::with_capacity(loaded.journals.len());
    for journal in &loaded.journals {
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
    for promotion in &loaded.temp_promotions {
        complete_journal_temp_promotion(paths, promotion)?;
    }
    for (journal, disposition) in loaded.journals.into_iter().zip(pending) {
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
    for operation_id in loaded.retired_operation_ids {
        remove_journal(paths, &operation_id)?;
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
    let retained_imports = retained_import_backups(&spec.settings_after, paths);
    for (index, mutation) in spec.directory_mutations.iter().enumerate() {
        if optional_directory_hash_recovery(&directory_temp_path(spec, paths, index)?)?.is_some() {
            return Err(recovery_evidence_error());
        }
        validate_cleanup_backup_evidence(
            &mutation.backup,
            mutation.expected_before_hash.as_deref(),
            directory_backup_is_retained(mutation, &retained_imports)?,
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
            let retained = retained_imports.contains(&lexical_absolute(backup)?);
            validate_cleanup_backup_evidence(backup, expected, retained)?;
        }
    }
    Ok(())
}

fn validate_cleanup_backup_evidence(
    backup: &Path,
    expected_hash: Option<&str>,
    retained: bool,
) -> Result<(), SkillError> {
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

fn load_and_validate_all_journals(paths: &SkillsPaths) -> Result<LoadedJournals, SkillError> {
    let root = paths.journals_skills_dir();
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(LoadedJournals {
                journals: Vec::new(),
                temp_promotions: Vec::new(),
                retired_operation_ids: Vec::new(),
            });
        }
        Err(error) => return Err(io_error(&root, error)),
    };
    let mut paths_by_id = std::collections::BTreeMap::<String, JournalFileSet>::new();
    let mut journal_files = 0_u64;
    let mut journal_bytes = 0_u64;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(&root, error))?;
        journal_files = journal_files.saturating_add(1);
        if journal_files > MAX_PENDING_JOURNAL_FILES {
            return Err(recovery_evidence_error());
        }
        let entry_path = entry.path();
        let metadata = fs::symlink_metadata(&entry_path).map_err(|_| recovery_evidence_error())?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_JOURNAL_BYTES {
            return Err(recovery_evidence_error());
        }
        journal_bytes = journal_bytes
            .checked_add(metadata.len())
            .ok_or_else(recovery_evidence_error)?;
        if journal_bytes > MAX_PENDING_JOURNAL_BYTES {
            return Err(recovery_evidence_error());
        }
        let file_name = entry
            .file_name()
            .into_string()
            .map_err(|_| recovery_evidence_error())?;
        enum FileState {
            Active,
            Temporary,
            Retiring,
            Retired,
        }
        let (operation_id, state) = if let Some(operation_id) = file_name
            .strip_prefix('.')
            .and_then(|value| value.strip_suffix(".json.tmp"))
        {
            (operation_id, FileState::Temporary)
        } else if let Some(operation_id) = file_name.strip_suffix(".json") {
            (operation_id, FileState::Active)
        } else if let Some(operation_id) = file_name.strip_suffix(".retiring") {
            (operation_id, FileState::Retiring)
        } else if let Some(operation_id) = file_name.strip_suffix(".retired") {
            (operation_id, FileState::Retired)
        } else {
            return Err(recovery_evidence_error());
        };
        validate_operation_id(operation_id).map_err(|_| recovery_evidence_error())?;
        let slot = paths_by_id.entry(operation_id.to_owned()).or_default();
        let path = match state {
            FileState::Active => &mut slot.active,
            FileState::Temporary => &mut slot.temporary,
            FileState::Retiring => &mut slot.retiring,
            FileState::Retired => &mut slot.retired,
        };
        if path.replace(entry_path).is_some() {
            return Err(recovery_evidence_error());
        }
    }

    let mut journals = Vec::with_capacity(paths_by_id.len());
    let mut temp_promotions = Vec::new();
    let mut retired_operation_ids = Vec::new();
    for (operation_id, files) in paths_by_id {
        if files.retired.is_some()
            && (files.active.is_some() || files.temporary.is_some() || files.retiring.is_some())
        {
            return Err(recovery_evidence_error());
        }
        if files.retiring.is_some() && (files.active.is_some() || files.temporary.is_some()) {
            return Err(recovery_evidence_error());
        }
        if let Some(retired) = files.retired {
            validate_retirement_journal(&retired, &operation_id)?;
            retired_operation_ids.push(operation_id);
            continue;
        }
        if let Some(retiring) = files.retiring {
            let journal = read_journal(&retiring)?;
            if journal.spec.operation_id != operation_id {
                return Err(recovery_evidence_error());
            }
            validate_transaction_bounds(&journal.spec, paths)?;
            journals.push(journal);
            continue;
        }
        let (journal, promotion) = match (files.active, files.temporary) {
            (Some(destination), Some(temporary)) => {
                let final_journal = read_journal(&destination)?;
                let temp_journal = read_journal(&temporary)?;
                if final_journal.spec != temp_journal.spec {
                    return Err(recovery_evidence_error());
                }
                (
                    temp_journal.clone(),
                    Some(JournalTempPromotion {
                        temporary,
                        destination,
                        journal: temp_journal,
                    }),
                )
            }
            (Some(destination), None) => (read_journal(&destination)?, None),
            (None, Some(temporary)) => {
                let temp_journal = read_journal(&temporary)?;
                let destination = journal_path(paths, &operation_id)?;
                (
                    temp_journal.clone(),
                    Some(JournalTempPromotion {
                        temporary,
                        destination,
                        journal: temp_journal,
                    }),
                )
            }
            (None, None) => unreachable!(),
        };
        if journal.spec.operation_id != operation_id {
            return Err(recovery_evidence_error());
        }
        validate_transaction_bounds(&journal.spec, paths)?;
        journals.push(journal);
        if let Some(promotion) = promotion {
            temp_promotions.push(promotion);
        }
    }
    Ok(LoadedJournals {
        journals,
        temp_promotions,
        retired_operation_ids,
    })
}

fn complete_journal_temp_promotion(
    paths: &SkillsPaths,
    promotion: &JournalTempPromotion,
) -> Result<(), SkillError> {
    validate_transaction_roots(paths, false)?;
    if read_journal(&promotion.temporary)? != promotion.journal {
        return Err(recovery_evidence_error());
    }
    match fs::symlink_metadata(&promotion.destination) {
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&promotion.destination, error)),
        Ok(_) => {
            let current = read_journal(&promotion.destination)?;
            if current.spec != promotion.journal.spec {
                return Err(recovery_evidence_error());
            }
        }
    }
    fs::rename(&promotion.temporary, &promotion.destination)
        .map_err(|_| recovery_evidence_error())?;
    sync_directory(&paths.journals_skills_dir())
}

#[doc(hidden)]
pub fn crash_transaction_at_phase_for_test(
    spec: TransactionSpec,
    phase: JournalPhase,
) -> Result<(), SkillError> {
    let paths = SkillsPaths::resolve_from_env()?;
    paths.ensure_mux_root()?;
    let _lock = acquire_skills_lock(&paths)?;
    paths.ensure_transaction_roots()?;
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
    let paths = SkillsPaths::resolve_from_env()?;
    paths.ensure_mux_root()?;
    let _lock = acquire_skills_lock(&paths)?;
    paths.ensure_transaction_roots()?;
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
        if !staging_tree_is_plain(&path) {
            continue;
        }
        #[cfg(unix)]
        {
            let expected = AnchoredRoot::inspect_directory(&path)?;
            let quarantine = root.join(format!(".mux-stale-{operation_id}.tmp"));
            ensure_missing(
                &quarantine,
                "a stale Skills staging quarantine requires recovery",
            )?;
            rename_same_parent_noreplace(&path, &quarantine)?;
            let actual = AnchoredRoot::inspect_directory(&quarantine)?;
            if actual.device != expected.device
                || actual.inode != expected.inode
                || actual.kind != expected.kind
            {
                let _ = rename_same_parent_noreplace(&quarantine, &path);
                continue;
            }
            if !staging_tree_is_plain(&quarantine) {
                let _ = rename_same_parent_noreplace(&quarantine, &path);
                continue;
            }
            remove_plain_staging_tree(&quarantine)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn staging_tree_is_plain(path: &Path) -> bool {
    fn inspect(anchor: &AnchoredRoot, directory: &File, path: &Path) -> Result<bool, SkillError> {
        use std::os::unix::ffi::OsStrExt;

        for name in anchor.read_directory(directory, path)? {
            let child = path.join(std::ffi::OsStr::from_bytes(name.to_bytes()));
            let identity = anchor.stat_entry(directory, &name, &child)?;
            match identity.kind {
                AnchoredFileKind::Regular if identity.links == 1 => {}
                AnchoredFileKind::Directory => {
                    let opened =
                        anchor.open_directory_entry(directory, &name, &identity, &child)?;
                    if !inspect(anchor, &opened, &child)? {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    let result = (|| {
        let anchor = AnchoredRoot::open(path)?;
        let directory = anchor.root_directory()?;
        inspect(&anchor, &directory, path)
    })();
    matches!(result, Ok(true))
}

#[cfg(unix)]
fn remove_plain_staging_tree(path: &Path) -> Result<(), SkillError> {
    use std::os::unix::ffi::OsStrExt;

    fn remove_children(
        anchor: &AnchoredRoot,
        directory: &File,
        path: &Path,
    ) -> Result<(), SkillError> {
        use std::os::unix::ffi::OsStrExt;

        for name in anchor.read_directory(directory, path)? {
            let child = path.join(std::ffi::OsStr::from_bytes(name.to_bytes()));
            let identity = anchor.stat_entry(directory, &name, &child)?;
            match identity.kind {
                AnchoredFileKind::Regular if identity.links == 1 => {
                    anchor.unlink_entry(directory, &name, false, &child)?;
                }
                AnchoredFileKind::Directory => {
                    let opened =
                        anchor.open_directory_entry(directory, &name, &identity, &child)?;
                    remove_children(anchor, &opened, &child)?;
                    drop(opened);
                    anchor.unlink_entry(directory, &name, true, &child)?;
                }
                _ => return Err(recovery_evidence_error()),
            }
        }
        Ok(())
    }

    let anchor = AnchoredRoot::open(path)?;
    let directory = anchor.root_directory()?;
    remove_children(&anchor, &directory, path)?;
    drop(directory);
    let parent_path = path.parent().ok_or_else(unsafe_transaction_path)?;
    let parent = AnchoredRoot::open(parent_path)?;
    let parent_directory = parent.root_directory()?;
    let name = std::ffi::CString::new(
        path.file_name()
            .ok_or_else(unsafe_transaction_path)?
            .as_bytes(),
    )
    .map_err(|_| unsafe_transaction_path())?;
    parent.unlink_entry(&parent_directory, &name, true, path)
}

#[cfg(not(unix))]
fn staging_tree_is_plain(_path: &Path) -> bool {
    false
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
    use crate::skills::{
        ManagedSkillRecord, RiskLevel, SkillContentKind, SkillRiskSummary, SkillSettingsSnapshot,
        SkillSource, SkillUpdateState, SkillsPaths, TransactionOrder, TransactionSpec,
    };
    use crate::testenv::TestHome;
    use std::collections::BTreeMap;
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
    fn directory_backup_retention_is_explicit_and_legacy_defaults_to_cleanup() {
        let legacy = serde_json::json!({
            "replacement": null,
            "destination": "/tmp/destination",
            "backup": "/tmp/backup",
            "expected_before_hash": "reviewed"
        });
        let mut mutation: DirectoryMutation = serde_json::from_value(legacy).unwrap();
        assert!(!mutation.retain_backup);

        mutation.retain_backup = true;
        assert_eq!(
            serde_json::to_value(mutation).unwrap()["retain_backup"],
            serde_json::Value::Bool(true)
        );
    }

    #[test]
    fn retained_directory_backup_requires_a_reviewed_before_hash() {
        let _home = TestHome::new("tx-retained-backup-bounds");
        let paths = SkillsPaths::from_env().unwrap();
        let mut spec = empty_spec("10500000-0000-4000-8000-000000000006");
        spec.directory_mutations.push(DirectoryMutation {
            replacement: None,
            destination: paths.central_skill("retained"),
            backup: paths.backups_skills_dir().join("retained/skill"),
            expected_before_hash: None,
            retain_backup: true,
        });

        assert!(matches!(
            validate_transaction_bounds(&spec, &paths),
            Err(SkillError::InvalidSource { .. })
        ));
    }

    #[test]
    fn backup_paths_must_not_be_ancestors_or_descendants_of_each_other() {
        let _home = TestHome::new("tx-overlapping-backups");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "11000000-0000-4000-8000-000000000006";
        let mut spec = empty_spec(id);
        let ancestor = paths.backups_skills_dir().join("reviewed-backup");
        spec.directory_mutations = vec![
            DirectoryMutation {
                replacement: None,
                destination: paths.central_skill("one"),
                backup: ancestor.clone(),
                expected_before_hash: None,
                retain_backup: false,
            },
            DirectoryMutation {
                replacement: None,
                destination: paths.central_skill("two"),
                backup: ancestor.join("nested"),
                expected_before_hash: None,
                retain_backup: false,
            },
        ];

        assert!(matches!(
            validate_transaction_bounds(&spec, &paths),
            Err(SkillError::InvalidSource { .. })
        ));
        assert!(!paths.backups_skills_dir().join("reviewed-backup").exists());
    }

    #[cfg(unix)]
    #[test]
    fn backup_paths_must_not_physically_overlap_through_a_symlink_alias() {
        use std::os::unix::fs::symlink;

        let _home = TestHome::new("tx-physical-backups");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "11000000-0000-4000-8000-000000000007";
        let physical_parent = paths.backups_skills_dir().join("physical");
        fs::create_dir(&physical_parent).unwrap();
        symlink(&physical_parent, paths.backups_skills_dir().join("alias")).unwrap();
        let mut spec = empty_spec(id);
        spec.directory_mutations = vec![
            DirectoryMutation {
                replacement: None,
                destination: paths.central_skill("one"),
                backup: physical_parent.join("reviewed"),
                expected_before_hash: None,
                retain_backup: false,
            },
            DirectoryMutation {
                replacement: None,
                destination: paths.central_skill("two"),
                backup: paths.backups_skills_dir().join("alias/reviewed/nested"),
                expected_before_hash: None,
                retain_backup: false,
            },
        ];

        assert!(matches!(
            validate_transaction_bounds(&spec, &paths),
            Err(SkillError::InvalidSource { .. })
        ));
        assert!(!physical_parent.join("reviewed").exists());
    }

    #[cfg(unix)]
    #[test]
    fn private_backup_parent_creation_never_mutates_through_a_symlink() {
        use std::os::unix::fs::symlink;

        let _home = TestHome::new("tx-backup-parent-link");
        let paths = SkillsPaths::from_env().unwrap();
        let owned = paths.backups_skills_dir().join("owned");
        fs::create_dir(&owned).unwrap();
        fs::write(owned.join("sentinel"), b"untouched").unwrap();
        symlink(&owned, paths.backups_skills_dir().join("alias")).unwrap();
        let backup = paths.backups_skills_dir().join("alias/new/skill");

        assert!(matches!(
            create_private_parent(&backup, &paths.backups_skills_dir()),
            Err(SkillError::UnsafePath { .. })
        ));
        assert_eq!(fs::read(owned.join("sentinel")).unwrap(), b"untouched");
        assert!(!owned.join("new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn execute_rejects_a_symlinked_control_ancestor_without_touching_its_target() {
        use std::os::unix::fs::{symlink, DirBuilderExt};

        let home = TestHome::new("tx-control-root-link");
        let mux = home.home.join(".mux");
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(&mux).unwrap();
        let outside = home.home.join("outside-journals");
        builder.mode(0o711).create(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"untouched").unwrap();
        symlink(&outside, mux.join("journals")).unwrap();

        let error =
            execute_transaction(empty_spec("11100000-0000-4000-8000-000000000006")).unwrap_err();

        assert!(matches!(
            error,
            SkillError::UnsafePath { .. } | SkillError::RecoveryRequired { .. }
        ));
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"untouched");
        assert_eq!(
            fs::read_dir(&outside)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>(),
            vec![OsString::from("sentinel")],
            "transaction setup must not create through a control-root symlink"
        );
        assert_eq!(
            fs::metadata(&outside).unwrap().permissions().mode() & 0o777,
            0o711
        );
    }

    #[cfg(unix)]
    #[test]
    fn recovery_rejects_a_symlinked_mux_home_without_touching_its_target() {
        use std::os::unix::fs::{symlink, DirBuilderExt};

        let home = TestHome::new("tx-mux-root-link");
        let outside = home.home.join("outside-mux");
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o711).create(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"untouched").unwrap();
        symlink(&outside, home.home.join(".mux")).unwrap();

        let error = recover_pending().unwrap_err();

        assert!(matches!(
            error,
            SkillError::UnsafePath { .. } | SkillError::RecoveryRequired { .. }
        ));
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"untouched");
        assert_eq!(
            fs::read_dir(&outside)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>(),
            vec![OsString::from("sentinel")],
            "recovery setup must not create through a symlinked MUX_HOME"
        );
        assert_eq!(
            fs::metadata(&outside).unwrap().permissions().mode() & 0o777,
            0o711
        );
    }

    #[test]
    fn retained_import_backup_must_still_match_its_reviewed_hash() {
        let _home = TestHome::new("tx-retained-backup-evidence");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "12000000-0000-4000-8000-000000000006";
        let backup = paths.backups_skills_dir().join("imported").join("skill");
        fs::create_dir_all(&backup).unwrap();
        fs::write(backup.join("original"), b"reviewed").unwrap();
        let expected_hash = hash_tree(&backup).unwrap();
        fs::write(backup.join("original"), b"changed after review").unwrap();

        let mut spec = empty_spec(id);
        spec.directory_mutations.push(DirectoryMutation {
            replacement: None,
            destination: paths.central_skill("skill"),
            backup: backup.clone(),
            expected_before_hash: Some(expected_hash),
            retain_backup: false,
        });
        spec.settings_after.managed_skills = Some(BTreeMap::from([(
            "skill".into(),
            ManagedSkillRecord {
                name: "skill".into(),
                description: "fixture".into(),
                content_kind: SkillContentKind::Instructions,
                source: SkillSource::Imported {
                    original_path: "~/.agents/skills/skill".into(),
                    backup_path: backup.to_string_lossy().into_owned(),
                },
                resolved_revision: None,
                content_hash: "unused".into(),
                installed_at: "2026-07-17T00:00:00Z".into(),
                updated_at: "2026-07-17T00:00:00Z".into(),
                risk: SkillRiskSummary {
                    level: RiskLevel::Low,
                    findings: Vec::new(),
                    finding_count: 0,
                    findings_truncated: false,
                },
                update: SkillUpdateState::default(),
            },
        )]));

        assert!(matches!(
            cleanup_obsolete_backups(&spec, &paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(backup.exists());
    }

    #[test]
    fn explicit_retained_backup_survives_successful_cleanup() {
        let _home = TestHome::new("tx-explicit-retained-backup");
        let paths = SkillsPaths::from_env().unwrap();
        let backup = paths.backups_skills_dir().join("retained/skill");
        fs::create_dir_all(&backup).unwrap();
        fs::write(backup.join("original"), b"reviewed").unwrap();
        let expected_hash = hash_tree(&backup).unwrap();
        let mut spec = empty_spec("12100000-0000-4000-8000-000000000006");
        spec.directory_mutations.push(DirectoryMutation {
            replacement: None,
            destination: paths.central_skill("retained"),
            backup: backup.clone(),
            expected_before_hash: Some(expected_hash.clone()),
            retain_backup: true,
        });

        cleanup_obsolete_backups(&spec, &paths).unwrap();

        assert_eq!(hash_tree(&backup).unwrap(), expected_hash);
    }

    #[test]
    fn non_retained_backup_is_removed_after_successful_cleanup() {
        let _home = TestHome::new("tx-non-retained-backup");
        let paths = SkillsPaths::from_env().unwrap();
        let backup = paths.backups_skills_dir().join("cleanup/skill");
        fs::create_dir_all(&backup).unwrap();
        fs::write(backup.join("original"), b"reviewed").unwrap();
        let expected_hash = hash_tree(&backup).unwrap();
        let mut spec = empty_spec("12100000-0000-4000-8000-000000000007");
        spec.directory_mutations.push(DirectoryMutation {
            replacement: None,
            destination: paths.central_skill("cleanup"),
            backup: backup.clone(),
            expected_before_hash: Some(expected_hash),
            retain_backup: false,
        });

        cleanup_obsolete_backups(&spec, &paths).unwrap();

        assert!(!backup.exists());
    }

    #[test]
    fn retained_backup_must_exist_with_the_reviewed_directory_type_and_hash() {
        let _home = TestHome::new("tx-retained-backup-exact-evidence");
        let paths = SkillsPaths::from_env().unwrap();
        let reviewed = paths.backups_skills_dir().join("reviewed-source");
        fs::create_dir_all(&reviewed).unwrap();
        fs::write(reviewed.join("original"), b"reviewed").unwrap();
        let expected_hash = hash_tree(&reviewed).unwrap();
        fs::remove_dir_all(&reviewed).unwrap();
        let backup = paths.backups_skills_dir().join("retained-exact/skill");
        let mut spec = empty_spec("12100000-0000-4000-8000-000000000008");
        spec.directory_mutations.push(DirectoryMutation {
            replacement: None,
            destination: paths.central_skill("retained-exact"),
            backup: backup.clone(),
            expected_before_hash: Some(expected_hash),
            retain_backup: true,
        });

        assert!(matches!(
            cleanup_obsolete_backups(&spec, &paths),
            Err(SkillError::RecoveryRequired { .. })
        ));

        fs::create_dir_all(&backup).unwrap();
        fs::write(backup.join("original"), b"drifted").unwrap();
        assert!(matches!(
            cleanup_obsolete_backups(&spec, &paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(backup.exists());

        fs::remove_dir_all(&backup).unwrap();
        fs::write(&backup, b"wrong type").unwrap();
        assert!(matches!(
            cleanup_obsolete_backups(&spec, &paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(backup.is_file());
    }

    #[test]
    fn retained_evidence_is_validated_before_any_obsolete_backup_is_deleted() {
        let _home = TestHome::new("tx-retained-backup-validation-order");
        let paths = SkillsPaths::from_env().unwrap();
        let obsolete = paths.backups_skills_dir().join("obsolete/skill");
        fs::create_dir_all(&obsolete).unwrap();
        fs::write(obsolete.join("original"), b"reviewed").unwrap();
        let obsolete_hash = hash_tree(&obsolete).unwrap();
        let retained = paths.backups_skills_dir().join("missing-retained/skill");
        let mut spec = empty_spec("12100000-0000-4000-8000-000000000009");
        spec.directory_mutations = vec![
            DirectoryMutation {
                replacement: None,
                destination: paths.central_skill("obsolete"),
                backup: obsolete.clone(),
                expected_before_hash: Some(obsolete_hash),
                retain_backup: false,
            },
            DirectoryMutation {
                replacement: None,
                destination: paths.central_skill("missing-retained"),
                backup: retained,
                expected_before_hash: Some("reviewed-but-missing".into()),
                retain_backup: true,
            },
        ];

        assert!(matches!(
            cleanup_obsolete_backups(&spec, &paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(obsolete.exists());
    }

    #[cfg(unix)]
    #[test]
    fn catalog_authority_rejects_an_arbitrary_symlink_alias() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("tx-catalog-alias");
        let authoritative = home.home.join(".agents/skills");
        fs::create_dir_all(&authoritative).unwrap();
        let alias = home.home.join("alias-skills");
        symlink(&authoritative, &alias).unwrap();
        let targets = vec![VerifiedCatalogTarget {
            lexical: lexical_absolute(&authoritative).unwrap(),
            canonical: canonicalize_deepest(&authoritative).unwrap(),
        }];

        assert!(matches!(
            validate_link_path(&alias.join("demo"), &targets),
            Err(SkillError::UnsafePath { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn catalog_aliases_cannot_mutate_the_same_physical_child_twice() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("tx-catalog-physical-dedup");
        let paths = SkillsPaths::from_env().unwrap();
        let agents = home.home.join(".agents/skills");
        fs::create_dir_all(&agents).unwrap();
        fs::create_dir(home.home.join(".cursor")).unwrap();
        symlink(&agents, home.home.join(".cursor/skills")).unwrap();
        let mut spec = empty_spec("12000000-0000-4000-8000-000000000007");
        spec.link_mutations = vec![
            LinkMutation {
                path: agents.join("shared"),
                expected: LinkState::Missing,
                desired_target: None,
                backup: None,
            },
            LinkMutation {
                path: home.home.join(".cursor/skills/shared"),
                expected: LinkState::Missing,
                desired_target: None,
                backup: None,
            },
        ];

        assert!(matches!(
            validate_transaction_bounds(&spec, &paths),
            Err(SkillError::InvalidSource { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn link_removal_never_deletes_a_different_entry_swapped_after_the_final_check() {
        use std::os::unix::fs::symlink;
        use std::sync::{Arc, Barrier};

        let home = TestHome::new("tx-link-swap-race");
        let paths = SkillsPaths::from_env().unwrap();
        let target_root = home.home.join(".agents/skills");
        fs::create_dir_all(&target_root).unwrap();
        let link = target_root.join("raced");
        symlink("missing-a", &link).unwrap();
        let mut spec = empty_spec("12100000-0000-4000-8000-000000000006");
        let mutation = LinkMutation {
            path: link.clone(),
            expected: LinkState::BrokenSymlink {
                target: PathBuf::from("missing-a"),
            },
            desired_target: None,
            backup: None,
        };
        spec.link_mutations.push(mutation.clone());

        let barrier = Arc::new(Barrier::new(2));
        let worker_barrier = Arc::clone(&barrier);
        let worker_link = link.clone();
        let worker = thread::spawn(move || {
            worker_barrier.wait();
            fs::remove_file(&worker_link).unwrap();
            symlink("missing-b", &worker_link).unwrap();
            worker_barrier.wait();
        });
        let mut hook = || {
            barrier.wait();
            barrier.wait();
        };

        let result = apply_link_with_hook(&spec, &paths, &mutation, 0, Some(&mut hook));
        worker.join().unwrap();

        assert!(matches!(result, Err(SkillError::PlanStale { .. })));
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("missing-b"));
        assert!(!link_temp_path(&spec, &mutation, 0).exists());
    }

    #[test]
    fn central_directory_swap_restores_an_unreviewed_tree_before_returning_stale() {
        use std::sync::{Arc, Barrier};

        let _home = TestHome::new("tx-directory-swap-race");
        let paths = SkillsPaths::from_env().unwrap();
        let destination = paths.central_skill("raced");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("value"), b"reviewed-a").unwrap();
        let reviewed_hash = hash_tree(&destination).unwrap();
        let mutation = DirectoryMutation {
            replacement: None,
            destination: destination.clone(),
            backup: paths.backups_skills_dir().join("race/raced"),
            expected_before_hash: Some(reviewed_hash),
            retain_backup: false,
        };
        let mut spec = empty_spec("12200000-0000-4000-8000-000000000006");
        spec.directory_mutations.push(mutation.clone());

        let barrier = Arc::new(Barrier::new(2));
        let worker_barrier = Arc::clone(&barrier);
        let worker_destination = destination.clone();
        let worker = thread::spawn(move || {
            worker_barrier.wait();
            fs::remove_dir_all(&worker_destination).unwrap();
            fs::create_dir(&worker_destination).unwrap();
            fs::write(worker_destination.join("value"), b"unreviewed-b").unwrap();
            worker_barrier.wait();
        });
        let mut hook = || {
            barrier.wait();
            barrier.wait();
        };

        let result = apply_directory_with_hook(&spec, &paths, &mutation, 0, Some(&mut hook));
        worker.join().unwrap();

        assert!(matches!(result, Err(SkillError::PlanStale { .. })));
        assert_eq!(
            fs::read(destination.join("value")).unwrap(),
            b"unreviewed-b"
        );
        assert!(!mutation.backup.exists());
    }

    #[cfg(unix)]
    #[test]
    fn link_exchange_restores_a_different_entry_swapped_after_the_final_check() {
        use std::os::unix::fs::symlink;
        use std::sync::{Arc, Barrier};

        let home = TestHome::new("tx-link-exchange-race");
        let paths = SkillsPaths::from_env().unwrap();
        let target_root = home.home.join(".agents/skills");
        fs::create_dir_all(&target_root).unwrap();
        let central = paths.central_skill("raced");
        fs::create_dir(&central).unwrap();
        fs::write(central.join("SKILL.md"), b"desired").unwrap();
        let link = target_root.join("raced");
        symlink("missing-a", &link).unwrap();
        let mutation = LinkMutation {
            path: link.clone(),
            expected: LinkState::BrokenSymlink {
                target: PathBuf::from("missing-a"),
            },
            desired_target: Some(central),
            backup: None,
        };
        let mut spec = empty_spec("12300000-0000-4000-8000-000000000006");
        spec.link_mutations.push(mutation.clone());

        let barrier = Arc::new(Barrier::new(2));
        let worker_barrier = Arc::clone(&barrier);
        let worker_link = link.clone();
        let worker = thread::spawn(move || {
            worker_barrier.wait();
            fs::remove_file(&worker_link).unwrap();
            symlink("missing-b", &worker_link).unwrap();
            worker_barrier.wait();
        });
        let mut hook = || {
            barrier.wait();
            barrier.wait();
        };

        let result = apply_link_with_hook(&spec, &paths, &mutation, 0, Some(&mut hook));
        worker.join().unwrap();

        assert!(matches!(result, Err(SkillError::PlanStale { .. })));
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("missing-b"));
        assert!(!link_temp_path(&spec, &mutation, 0).exists());
    }

    #[test]
    fn recovery_restores_an_opaque_quarantine_instead_of_deleting_it() {
        let home = TestHome::new("tx-opaque-link-temp");
        let paths = SkillsPaths::from_env().unwrap();
        let target_root = home.home.join(".agents/skills");
        fs::create_dir_all(&target_root).unwrap();
        let mutation = LinkMutation {
            path: target_root.join("opaque"),
            expected: LinkState::Missing,
            desired_target: None,
            backup: None,
        };
        let spec = empty_spec("12400000-0000-4000-8000-000000000006");
        let temporary = link_temp_path(&spec, &mutation, 0);
        fs::write(&temporary, b"concurrent-user-entry").unwrap();

        assert!(matches!(
            cleanup_link_temporary(&temporary, &mutation, &paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert_eq!(fs::read(&mutation.path).unwrap(), b"concurrent-user-entry");
        assert!(!temporary.exists());
    }

    #[cfg(unix)]
    #[test]
    fn opaque_quarantine_replaces_only_an_exact_desired_link_and_preserves_third_state() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("tx-opaque-link-states");
        let paths = SkillsPaths::from_env().unwrap();
        let target_root = home.home.join(".agents/skills");
        fs::create_dir_all(&target_root).unwrap();
        let central = paths.central_skill("opaque");
        fs::create_dir(&central).unwrap();
        let mutation = LinkMutation {
            path: target_root.join("opaque"),
            expected: LinkState::Missing,
            desired_target: Some(central.clone()),
            backup: None,
        };
        let spec = empty_spec("12500000-0000-4000-8000-000000000006");
        let temporary = link_temp_path(&spec, &mutation, 0);
        fs::write(&temporary, b"opaque-user-entry").unwrap();
        symlink(&central, &mutation.path).unwrap();

        assert!(matches!(
            cleanup_link_temporary(&temporary, &mutation, &paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert_eq!(fs::read(&mutation.path).unwrap(), b"opaque-user-entry");
        assert!(!temporary.exists());

        fs::remove_file(&mutation.path).unwrap();
        fs::write(&mutation.path, b"third-state").unwrap();
        fs::write(&temporary, b"second-opaque-entry").unwrap();
        assert!(matches!(
            cleanup_link_temporary(&temporary, &mutation, &paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert_eq!(fs::read(&mutation.path).unwrap(), b"third-state");
        assert_eq!(fs::read(&temporary).unwrap(), b"second-opaque-entry");
    }

    #[test]
    fn recovery_restores_an_opaque_directory_backup_to_an_empty_original_slot() {
        let _home = TestHome::new("tx-opaque-directory-backup");
        let paths = SkillsPaths::from_env().unwrap();
        let destination = paths.central_skill("opaque");
        let reviewed = paths.central_skill("reviewed-fixture");
        fs::create_dir(&reviewed).unwrap();
        fs::write(reviewed.join("value"), b"reviewed-a").unwrap();
        let reviewed_hash = hash_tree(&reviewed).unwrap();
        fs::remove_dir_all(&reviewed).unwrap();
        let backup = paths.backups_skills_dir().join("opaque/skill");
        create_private_parent(&backup, &paths.backups_skills_dir()).unwrap();
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("value"), b"concurrent-b").unwrap();
        let mutation = DirectoryMutation {
            replacement: None,
            destination: destination.clone(),
            backup: backup.clone(),
            expected_before_hash: Some(reviewed_hash),
            retain_backup: false,
        };
        let mut spec = empty_spec("12600000-0000-4000-8000-000000000006");
        spec.directory_mutations.push(mutation);
        write_journal(&paths, &spec, JournalPhase::Prepared).unwrap();

        assert!(matches!(
            recover_pending_with_paths(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert_eq!(
            fs::read(destination.join("value")).unwrap(),
            b"concurrent-b"
        );
        assert!(!backup.exists());
        assert!(journal_path(&paths, &spec.operation_id).unwrap().exists());
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

    #[test]
    fn recovery_completes_a_fsynced_journal_temp_left_before_rename() {
        let _home = TestHome::new("tx-journal-temp-crash");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "21000000-0000-4000-8000-000000000006";
        let spec = empty_spec(id);
        create_private_journal_root(&paths).unwrap();
        let temporary = journal_temp_path(&paths, id).unwrap();
        let bytes = serde_json::to_vec(&Journal {
            version: JOURNAL_SCHEMA_VERSION,
            spec,
            phase: JournalPhase::Prepared,
        })
        .unwrap();
        let mut file = create_private_new_file(&temporary).unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        drop(file);

        recover_pending_with_paths(&paths).unwrap();

        assert!(!temporary.exists());
        assert!(!paths.journals_skills_dir().exists());
        recover_pending_with_paths(&paths).unwrap();
    }

    #[test]
    fn recovery_retains_a_malformed_owned_journal_temp_for_manual_review() {
        let _home = TestHome::new("tx-malformed-journal-temp");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "21100000-0000-4000-8000-000000000006";
        let temporary = journal_temp_path(&paths, id).unwrap();
        let mut file = create_private_new_file(&temporary).unwrap();
        file.write_all(b"{malformed").unwrap();
        file.sync_all().unwrap();
        drop(file);

        assert!(matches!(
            recover_pending_with_paths(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(temporary.exists());
    }

    #[test]
    fn recovery_rejects_different_specs_in_final_and_temp_journal_files() {
        let _home = TestHome::new("tx-colliding-journal-temp");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "21200000-0000-4000-8000-000000000006";
        write_journal(&paths, &empty_spec(id), JournalPhase::Prepared).unwrap();
        let temporary = journal_temp_path(&paths, id).unwrap();
        let mut different = empty_spec(id);
        different.settings_after.skill_update_checked_at = Some("different".into());
        let bytes = serde_json::to_vec(&Journal {
            version: JOURNAL_SCHEMA_VERSION,
            spec: different,
            phase: JournalPhase::ContentSwapped,
        })
        .unwrap();
        let mut file = create_private_new_file(&temporary).unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        drop(file);

        assert!(matches!(
            recover_pending_with_paths(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(temporary.exists());
        assert!(journal_path(&paths, id).unwrap().exists());
    }

    #[test]
    fn recovery_promotes_a_same_spec_temp_phase_over_the_final_journal() {
        let _home = TestHome::new("tx-advanced-journal-temp");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "21300000-0000-4000-8000-000000000006";
        let spec = empty_spec(id);
        write_journal(&paths, &spec, JournalPhase::Prepared).unwrap();
        let temporary = journal_temp_path(&paths, id).unwrap();
        let bytes = serde_json::to_vec(&Journal {
            version: JOURNAL_SCHEMA_VERSION,
            spec,
            phase: JournalPhase::SettingsWritten,
        })
        .unwrap();
        let mut file = create_private_new_file(&temporary).unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        drop(file);

        recover_pending_with_paths(&paths).unwrap();

        assert!(!temporary.exists());
        assert!(!journal_path(&paths, id).unwrap().exists());
    }

    #[test]
    fn recovery_rejects_unknown_fields_inside_the_journal_spec() {
        let _home = TestHome::new("tx-journal-nested-field");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "22000000-0000-4000-8000-000000000006";
        let mut value = serde_json::to_value(Journal {
            version: JOURNAL_SCHEMA_VERSION,
            spec: empty_spec(id),
            phase: JournalPhase::Prepared,
        })
        .unwrap();
        value["spec"]["settings_before"]["unreviewed"] = serde_json::Value::Bool(true);
        let path = journal_path(&paths, id).unwrap();
        let mut file = create_private_new_file(&path).unwrap();
        file.write_all(&serde_json::to_vec(&value).unwrap())
            .unwrap();
        file.sync_all().unwrap();
        drop(file);

        assert!(matches!(
            load_and_validate_all_journals(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(path.exists());
    }

    #[test]
    fn recovery_rejects_an_unsupported_journal_schema_version() {
        let _home = TestHome::new("tx-journal-version");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "22100000-0000-4000-8000-000000000006";
        let mut value = serde_json::to_value(Journal {
            version: JOURNAL_SCHEMA_VERSION,
            spec: empty_spec(id),
            phase: JournalPhase::Prepared,
        })
        .unwrap();
        value["version"] = serde_json::Value::from(JOURNAL_SCHEMA_VERSION + 1);
        let path = journal_path(&paths, id).unwrap();
        let mut file = create_private_new_file(&path).unwrap();
        file.write_all(&serde_json::to_vec(&value).unwrap())
            .unwrap();
        file.sync_all().unwrap();
        drop(file);

        assert!(matches!(
            load_and_validate_all_journals(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(path.exists());
    }

    #[test]
    fn recovery_bounds_pending_journal_file_count_before_parsing() {
        let _home = TestHome::new("tx-journal-count");
        let paths = SkillsPaths::from_env().unwrap();
        for index in 0..=MAX_PENDING_JOURNAL_FILES {
            let id = format!("23000000-0000-4000-8000-{index:012x}");
            let path = journal_path(&paths, &id).unwrap();
            let bytes = serde_json::to_vec(&Journal {
                version: JOURNAL_SCHEMA_VERSION,
                spec: empty_spec(&id),
                phase: JournalPhase::Prepared,
            })
            .unwrap();
            let mut file = create_private_new_file(&path).unwrap();
            file.write_all(&bytes).unwrap();
        }

        assert!(matches!(
            load_and_validate_all_journals(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert_eq!(
            fs::read_dir(paths.journals_skills_dir()).unwrap().count() as u64,
            MAX_PENDING_JOURNAL_FILES + 1
        );
    }

    #[test]
    fn recovery_bounds_aggregate_pending_journal_bytes_before_parsing() {
        let _home = TestHome::new("tx-journal-bytes");
        let paths = SkillsPaths::from_env().unwrap();
        let files = MAX_PENDING_JOURNAL_BYTES / MAX_JOURNAL_BYTES + 1;
        for index in 0..files {
            let id = format!("24000000-0000-4000-8000-{index:012x}");
            let path = journal_path(&paths, &id).unwrap();
            let mut bytes = serde_json::to_vec(&Journal {
                version: JOURNAL_SCHEMA_VERSION,
                spec: empty_spec(&id),
                phase: JournalPhase::Prepared,
            })
            .unwrap();
            bytes.resize(MAX_JOURNAL_BYTES as usize, b' ');
            let mut file = create_private_new_file(&path).unwrap();
            file.write_all(&bytes).unwrap();
        }

        assert!(matches!(
            load_and_validate_all_journals(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert_eq!(
            fs::read_dir(paths.journals_skills_dir()).unwrap().count() as u64,
            files
        );
    }

    #[test]
    fn a_durable_retiring_journal_remains_pending_and_recoverable() {
        let _home = TestHome::new("tx-journal-retiring");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "25000000-0000-4000-8000-000000000006";
        write_journal(&paths, &empty_spec(id), JournalPhase::Prepared).unwrap();

        assert!(matches!(
            remove_journal_with_failpoint(&paths, id, Some(JournalRetireFailpoint::RetiringSynced),),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(journal_retiring_path(&paths, id).unwrap().exists());
        assert!(has_pending_recovery_with_paths(&paths).unwrap());

        recover_pending_with_paths(&paths).unwrap();
        assert!(!has_pending_recovery_with_paths(&paths).unwrap());
        assert!(!journal_retiring_path(&paths, id).unwrap().exists());
    }

    #[test]
    fn a_durable_retired_journal_is_inert_and_recovery_only_deletes_it() {
        let _home = TestHome::new("tx-journal-retired");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "25100000-0000-4000-8000-000000000006";
        write_journal(&paths, &empty_spec(id), JournalPhase::Prepared).unwrap();

        assert!(matches!(
            remove_journal_with_failpoint(&paths, id, Some(JournalRetireFailpoint::RetiredSynced),),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(journal_retired_path(&paths, id).unwrap().exists());
        assert!(!has_pending_recovery_with_paths(&paths).unwrap());

        recover_pending_with_paths(&paths).unwrap();
        assert!(!journal_retired_path(&paths, id).unwrap().exists());
    }

    #[test]
    fn failure_after_retired_unlink_is_benign_and_a_resurrected_marker_is_inert() {
        let _home = TestHome::new("tx-journal-retired-unlink");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "25200000-0000-4000-8000-000000000006";
        write_journal(&paths, &empty_spec(id), JournalPhase::Prepared).unwrap();
        let bytes = fs::read(journal_path(&paths, id).unwrap()).unwrap();

        remove_journal_with_failpoint(
            &paths,
            id,
            Some(JournalRetireFailpoint::RetiredUnlinkedBeforeSync),
        )
        .unwrap();
        assert!(!has_pending_recovery_with_paths(&paths).unwrap());

        create_private_journal_root(&paths).unwrap();
        let resurrected = journal_retired_path(&paths, id).unwrap();
        let mut file = create_private_new_file(&resurrected).unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert!(!has_pending_recovery_with_paths(&paths).unwrap());
        recover_pending_with_paths(&paths).unwrap();
        assert!(!resurrected.exists());
    }

    #[test]
    fn active_and_retired_journals_for_one_operation_are_never_ambiguous() {
        let _home = TestHome::new("tx-journal-retired-collision");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "25300000-0000-4000-8000-000000000006";
        write_journal(&paths, &empty_spec(id), JournalPhase::Prepared).unwrap();
        fs::copy(
            journal_path(&paths, id).unwrap(),
            journal_retired_path(&paths, id).unwrap(),
        )
        .unwrap();
        #[cfg(unix)]
        fs::set_permissions(
            journal_retired_path(&paths, id).unwrap(),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();

        assert!(matches!(
            has_pending_recovery_with_paths(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(journal_path(&paths, id).unwrap().exists());
        assert!(journal_retired_path(&paths, id).unwrap().exists());
    }

    #[test]
    fn journal_writes_reject_an_inert_retirement_marker_for_the_same_operation() {
        let _home = TestHome::new("tx-journal-retired-write-collision");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "25310000-0000-4000-8000-000000000006";
        write_journal(&paths, &empty_spec(id), JournalPhase::Prepared).unwrap();
        assert!(matches!(
            remove_journal_with_failpoint(&paths, id, Some(JournalRetireFailpoint::RetiredSynced),),
            Err(SkillError::RecoveryRequired { .. })
        ));

        assert!(matches!(
            write_journal(&paths, &empty_spec(id), JournalPhase::Prepared),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert!(journal_retired_path(&paths, id).unwrap().exists());
        assert!(!journal_path(&paths, id).unwrap().exists());
        assert!(!journal_temp_path(&paths, id).unwrap().exists());
    }

    #[cfg(unix)]
    #[test]
    fn retired_journal_symlinks_are_rejected_and_never_followed() {
        use std::os::unix::fs::symlink;

        let home = TestHome::new("tx-retired-journal-link");
        let paths = SkillsPaths::from_env().unwrap();
        let id = "25400000-0000-4000-8000-000000000006";
        let outside = home.home.join("outside-retired-journal");
        fs::write(&outside, b"untouched").unwrap();
        symlink(&outside, journal_retired_path(&paths, id).unwrap()).unwrap();

        assert!(matches!(
            has_pending_recovery_with_paths(&paths),
            Err(SkillError::RecoveryRequired { .. })
        ));
        assert_eq!(fs::read(&outside).unwrap(), b"untouched");
        assert!(
            fs::symlink_metadata(journal_retired_path(&paths, id).unwrap())
                .unwrap()
                .file_type()
                .is_symlink()
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
        let nested_link = "40000000-0000-4000-8000-00000000000d";

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

        let nested_link_root = write_staging_case(
            &paths,
            nested_link,
            format!(r#"{{"operation_id":"{nested_link}","created_at":"2026-07-15T00:00:00Z"}}"#),
        );
        fs::create_dir(nested_link_root.join("content")).unwrap();
        symlink(&outside, nested_link_root.join("content/external")).unwrap();

        cleanup_abandoned_staging(&paths, now).unwrap();

        assert!(!paths.staging_skills_dir().join(stale).exists());
        for retained in [
            exact_day,
            malformed,
            wrong_id,
            linked_root,
            linked_metadata,
            journaled,
            nested_link,
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
