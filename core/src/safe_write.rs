use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);
const SETTINGS_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const SETTINGS_LOCK_POLL: Duration = Duration::from_millis(25);
const TRANSACTION_WRITE_RECORD_VERSION: u32 = 1;

pub(crate) struct SettingsLock {
    lock_path: PathBuf,
    _lock_file: Rc<fs::File>,
    _thread_bound: PhantomData<Rc<()>>,
}

/// A shared filesystem lock used by read-only workspace projections.
///
/// This guard is deliberately separate from [`SettingsLock`]: a projection
/// must coordinate with cooperating writers, but it must not make an empty
/// MUX home observable merely by reading it.
pub(crate) struct SettingsReadLock {
    _ownership: SettingsReadLockOwnership,
    _thread_bound: PhantomData<Rc<()>>,
}

enum SettingsReadLockOwnership {
    Reentrant { _lock: SettingsLock },
    Shared { _file: fs::File },
}

pub(crate) enum TrySettingsReadLock {
    Missing,
    Acquired(SettingsReadLock),
    Contended,
}

struct HeldSettingsLock {
    depth: usize,
    lock_file: Rc<fs::File>,
}

thread_local! {
    /// A full asset commit holds the settings filesystem lock across several
    /// nested settings mutations. Track ownership per thread so those nested
    /// calls are reentrant without letting another thread in this process bypass
    /// the cross-process lock.
    static HELD_SETTINGS_LOCKS: RefCell<BTreeMap<PathBuf, HeldSettingsLock>> =
        const { RefCell::new(BTreeMap::new()) };
    static ACTIVE_TRANSACTION_WRITES: RefCell<Option<Rc<RefCell<ActiveTransactionWrites>>>> =
        const { RefCell::new(None) };
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum TransactionPathState {
    Missing,
    File {
        bytes: Vec<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<u32>,
    },
    Symlink {
        target: PathBuf,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DurableTransactionWrite {
    version: u32,
    sequence: u64,
    path: PathBuf,
    state: TransactionPathState,
}

#[derive(Debug)]
struct ActiveTransactionWrites {
    directory: PathBuf,
    tracked_paths: BTreeSet<PathBuf>,
    states: BTreeMap<PathBuf, TransactionPathState>,
    next_sequence: u64,
}

/// Thread-bound ownership evidence for a central asset transaction.
///
/// Every successful safe write records the exact post-state before control
/// returns to the transaction coordinator. The record is also synced under the
/// durable rollback directory. A crash between publishing a target and syncing
/// its record therefore fails closed during recovery instead of guessing that
/// the current bytes belong to MUX.
pub(crate) struct TransactionWriteTracker {
    active: Rc<RefCell<ActiveTransactionWrites>>,
    _thread_bound: PhantomData<Rc<()>>,
}

impl TransactionWriteTracker {
    pub(crate) fn states(&self) -> BTreeMap<PathBuf, TransactionPathState> {
        self.active.borrow().states.clone()
    }
}

impl Drop for TransactionWriteTracker {
    fn drop(&mut self) {
        ACTIVE_TRANSACTION_WRITES.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot
                .as_ref()
                .is_some_and(|active| Rc::ptr_eq(active, &self.active))
            {
                slot.take();
            }
        });
    }
}

impl Drop for SettingsLock {
    fn drop(&mut self) {
        let released = HELD_SETTINGS_LOCKS.with(|held| {
            let mut held = held.borrow_mut();
            let entry = held.get_mut(&self.lock_path)?;
            entry.depth -= 1;
            if entry.depth == 0 {
                held.remove(&self.lock_path)
            } else {
                None
            }
        });
        if let Some(released) = released {
            let _ = FileExt::unlock(released.lock_file.as_ref());
        }
    }
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

/// Coordinate cooperating writers with an OS-backed advisory lock next to the
/// shared settings file. The lock file is intentionally persistent: the kernel
/// releases ownership if a process crashes, so no stale directory can brick all
/// future commits.
pub(crate) fn acquire_settings_lock(path: &Path) -> Result<SettingsLock, String> {
    let lock_path = append_suffix(path, ".lockfile");
    if let Some(reentered) = reenter_settings_lock(&lock_path) {
        return Ok(reentered);
    }

    let parent = lock_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let lock_file = open_settings_lock_file(&lock_path)?;
    let started = Instant::now();
    loop {
        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                let lock_file = Rc::new(lock_file);
                HELD_SETTINGS_LOCKS.with(|held| {
                    held.borrow_mut().insert(
                        lock_path.clone(),
                        HeldSettingsLock {
                            depth: 1,
                            lock_file: lock_file.clone(),
                        },
                    );
                });
                return Ok(SettingsLock {
                    _lock_file: lock_file,
                    lock_path,
                    _thread_bound: PhantomData,
                });
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if started.elapsed() >= SETTINGS_LOCK_TIMEOUT {
                    return Err(format!(
                        "refusing to modify {}: timed out waiting for the settings lock",
                        path.display()
                    ));
                }
                thread::sleep(SETTINGS_LOCK_POLL);
            }
            Err(error) => {
                return Err(format!(
                    "failed to acquire settings lock for {}: {}",
                    path.display(),
                    error
                ));
            }
        }
    }
}

fn reenter_settings_lock(lock_path: &Path) -> Option<SettingsLock> {
    let reentered = HELD_SETTINGS_LOCKS.with(|held| {
        let mut held = held.borrow_mut();
        let entry = held.get_mut(lock_path)?;
        entry.depth += 1;
        Some(entry.lock_file.clone())
    });
    reentered.map(|lock_file| SettingsLock {
        lock_path: lock_path.to_path_buf(),
        _lock_file: lock_file,
        _thread_bound: PhantomData,
    })
}

/// Acquire a shared settings lock only when MUX storage has already been
/// initialized. An existing settings document is enough to initialize the
/// persistent lock file; when neither file exists this returns `None` without
/// creating the parent directory.
///
/// Callers that receive `None` must recheck after completing their read. A
/// writer creates the lock file before publishing settings, so appearance of
/// either path means the read should be retried under the shared lock.
#[cfg(test)]
pub(crate) fn acquire_settings_read_lock_if_initialized(
    path: &Path,
) -> Result<Option<SettingsReadLock>, String> {
    let started = Instant::now();
    loop {
        match try_acquire_settings_read_lock_if_initialized(path)? {
            TrySettingsReadLock::Missing => return Ok(None),
            TrySettingsReadLock::Acquired(lock) => return Ok(Some(lock)),
            TrySettingsReadLock::Contended if started.elapsed() < SETTINGS_LOCK_TIMEOUT => {
                thread::sleep(
                    SETTINGS_LOCK_POLL.min(SETTINGS_LOCK_TIMEOUT.saturating_sub(started.elapsed())),
                );
            }
            TrySettingsReadLock::Contended => {
                return Err(format!(
                    "refusing to read {}: timed out waiting for the settings lock",
                    path.display()
                ));
            }
        }
    }
}

/// Attempt the shared settings lock once without waiting. Workspace snapshots
/// use this together with the Skills try-lock so they never wait for one domain
/// while retaining the other domain's shared lock.
pub(crate) fn try_acquire_settings_read_lock_if_initialized(
    path: &Path,
) -> Result<TrySettingsReadLock, String> {
    let lock_path = append_suffix(path, ".lockfile");
    if let Some(lock) = reenter_settings_lock(&lock_path) {
        return Ok(TrySettingsReadLock::Acquired(SettingsReadLock {
            _ownership: SettingsReadLockOwnership::Reentrant { _lock: lock },
            _thread_bound: PhantomData,
        }));
    }

    if !settings_lock_is_initialized(path)? {
        return Ok(TrySettingsReadLock::Missing);
    }

    // `open_settings_lock_file` may create only the lock file. Reaching this
    // point proves either it or settings.json already existed, so a read of an
    // empty MUX home remains side-effect free.
    let lock_file = open_settings_lock_file(&lock_path)?;
    match FileExt::try_lock_shared(&lock_file) {
        Ok(()) => Ok(TrySettingsReadLock::Acquired(SettingsReadLock {
            _ownership: SettingsReadLockOwnership::Shared { _file: lock_file },
            _thread_bound: PhantomData,
        })),
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(TrySettingsReadLock::Contended),
        Err(error) => Err(format!(
            "failed to acquire shared settings lock for {}: {}",
            path.display(),
            error
        )),
    }
}

pub(crate) fn settings_lock_is_initialized(path: &Path) -> Result<bool, String> {
    let lock_path = append_suffix(path, ".lockfile");
    let lock_exists = match fs::symlink_metadata(&lock_path) {
        Ok(_) => true,
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(error) => {
            return Err(format!(
                "failed to inspect settings lock {}: {error}",
                lock_path.display()
            ));
        }
    };
    if lock_exists {
        return Ok(true);
    }
    let settings_exists = match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(error) => {
            return Err(format!(
                "failed to inspect settings path {}: {error}",
                path.display()
            ));
        }
    };
    Ok(settings_exists)
}

#[cfg(unix)]
fn open_settings_lock_file(path: &Path) -> Result<fs::File, String> {
    use rustix::fs::{openat, Mode, OFlags, CWD};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let file = openat(
        CWD,
        path,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::from(0o600),
    )
    .map(fs::File::from)
    .map_err(|error| format!("failed to open settings lock {}: {error}", path.display()))?;
    let metadata = file.metadata().map_err(|error| {
        format!(
            "failed to inspect settings lock {}: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(format!(
            "refusing unsafe settings lock file: {}",
            path.display()
        ));
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("failed to secure settings lock {}: {error}", path.display()))?;
    Ok(file)
}

#[cfg(not(unix))]
fn open_settings_lock_file(path: &Path) -> Result<fs::File, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            return Err(format!(
                "refusing unsafe settings lock file: {}",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .map_err(|error| format!("failed to open settings lock {}: {error}", path.display()))
}

pub(crate) fn begin_transaction_write_tracking(
    directory: &Path,
    tracked_paths: &[PathBuf],
) -> Result<TransactionWriteTracker, String> {
    let already_active = ACTIVE_TRANSACTION_WRITES.with(|slot| slot.borrow().is_some());
    if already_active {
        return Err("a transaction write tracker is already active on this thread".into());
    }
    match fs::create_dir(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(format!(
                "transaction write evidence directory already exists: {}",
                directory.display()
            ));
        }
        Err(error) => {
            return Err(format!(
                "failed to create transaction write evidence directory {}: {error}",
                directory.display()
            ));
        }
    }
    set_private_directory(directory)?;
    sync_parent(directory)?;
    let active = Rc::new(RefCell::new(ActiveTransactionWrites {
        directory: directory.to_path_buf(),
        tracked_paths: tracked_paths.iter().cloned().collect(),
        states: BTreeMap::new(),
        next_sequence: 0,
    }));
    ACTIVE_TRANSACTION_WRITES.with(|slot| {
        *slot.borrow_mut() = Some(active.clone());
    });
    Ok(TransactionWriteTracker {
        active,
        _thread_bound: PhantomData,
    })
}

pub(crate) fn load_transaction_write_states(
    directory: &Path,
) -> Result<BTreeMap<PathBuf, TransactionPathState>, String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(format!(
                "recovery_required: failed to read transaction write evidence {}: {error}",
                directory.display()
            ));
        }
    };
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("recovery_required: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut states = BTreeMap::new();
    for (expected_sequence, path) in paths.into_iter().enumerate() {
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| format!("recovery_required: {error}"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!(
                "recovery_required: invalid transaction write evidence {}",
                path.display()
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(format!(
                    "recovery_required: transaction write evidence is not private: {}",
                    path.display()
                ));
            }
        }
        let bytes = fs::read(&path).map_err(|error| format!("recovery_required: {error}"))?;
        let record: DurableTransactionWrite = serde_json::from_slice(&bytes).map_err(|_| {
            format!(
                "recovery_required: malformed transaction write evidence {}",
                path.display()
            )
        })?;
        if record.version != TRANSACTION_WRITE_RECORD_VERSION
            || record.sequence != expected_sequence as u64
            || path.file_name().and_then(|name| name.to_str())
                != Some(format!("{expected_sequence:020}.json").as_str())
        {
            return Err(format!(
                "recovery_required: inconsistent transaction write evidence {}",
                path.display()
            ));
        }
        states.insert(record.path, record.state);
    }
    Ok(states)
}

pub(crate) fn record_transaction_symlink(path: &Path, target: &Path) -> Result<(), String> {
    record_transaction_path_state(
        path,
        TransactionPathState::Symlink {
            target: target.to_path_buf(),
        },
    )
}

pub(crate) fn record_transaction_removal(path: &Path) -> Result<(), String> {
    record_transaction_path_state(path, TransactionPathState::Missing)
}

fn record_transaction_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mode = file_mode(path)?;
    record_transaction_path_state(
        path,
        TransactionPathState::File {
            bytes: bytes.to_vec(),
            mode,
        },
    )
}

fn record_transaction_path_state(path: &Path, state: TransactionPathState) -> Result<(), String> {
    let active = ACTIVE_TRANSACTION_WRITES.with(|slot| slot.borrow().clone());
    let Some(active) = active else {
        return Ok(());
    };
    let mut active = active.borrow_mut();
    if !active.tracked_paths.contains(path) {
        return Ok(());
    }
    // Keep the in-memory ownership evidence first. If syncing the durable
    // evidence fails, the current process can still roll back safely; a crash
    // in that interval will conservatively require manual recovery.
    active.states.insert(path.to_path_buf(), state.clone());
    let sequence = active.next_sequence;
    active.next_sequence += 1;
    let record = DurableTransactionWrite {
        version: TRANSACTION_WRITE_RECORD_VERSION,
        sequence,
        path: path.to_path_buf(),
        state,
    };
    let bytes = serde_json::to_vec(&record).map_err(|error| error.to_string())?;
    let record_path = active.directory.join(format!("{sequence:020}.json"));
    write_private_new_file(&record_path, &bytes)?;
    sync_parent(&record_path)
}

fn write_private_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("failed to sync {}: {error}", path.display()))
}

fn set_private_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn sync_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("failed to sync {}: {error}", parent.display()))
}

fn file_mode(path: &Path) -> Result<Option<u32>, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(Some(metadata.permissions().mode()))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(None)
    }
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read {}: {}", path.display(), error)),
    }
}

fn resolve_destination(path: &Path) -> Result<PathBuf, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::canonicalize(path)
            .map_err(|error| format!("failed to resolve {}: {}", path.display(), error)),
        Ok(_) => Ok(path.to_path_buf()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(format!("failed to inspect {}: {}", path.display(), error)),
    }
}

/// Atomically replace a text file only if it still contains the text that was
/// parsed by the caller. This avoids truncated files and refuses to overwrite a
/// concurrent edit made by the Agent or the user.
pub(crate) fn write_if_unchanged(
    path: &Path,
    expected: Option<&str>,
    content: &str,
) -> Result<(), String> {
    write_if_unchanged_impl(path, expected, content, false)
}

/// Settings may contain environment variables, request headers, and disabled
/// entry snapshots. Keep that MUX-owned store private while retaining all other
/// optimistic-concurrency and atomic-replace guarantees.
pub(crate) fn write_private_if_unchanged(
    path: &Path,
    expected: Option<&str>,
    content: &str,
) -> Result<(), String> {
    write_if_unchanged_impl(path, expected, content, true)
}

/// Remove a file only when its current contents still match what the caller
/// prepared against. Used to roll back a newly-created file in a multi-file
/// model configuration transaction.
pub(crate) fn remove_if_unchanged(path: &Path, expected: &str) -> Result<(), String> {
    if read_optional(path)?.as_deref() != Some(expected) {
        return Err(format!(
            "refusing to remove {} during rollback: file changed after MUX wrote it",
            path.display()
        ));
    }
    fs::remove_file(path)
        .map_err(|error| format!("failed to remove {}: {}", path.display(), error))?;
    record_transaction_removal(path)
}

#[derive(Debug, PartialEq, Eq)]
struct FileBytes {
    bytes: Vec<u8>,
    mode: Option<u32>,
}

fn read_optional_bytes(path: &Path) -> Result<Option<FileBytes>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing to treat symlink {} as a regular transaction file",
            path.display()
        )),
        Ok(metadata) if metadata.is_dir() => Err(format!(
            "refusing to treat directory {} as a transaction file",
            path.display()
        )),
        Ok(metadata) if metadata.is_file() => {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                Some(metadata.permissions().mode())
            };
            #[cfg(not(unix))]
            let mode = None;
            fs::read(path)
                .map(|bytes| Some(FileBytes { bytes, mode }))
                .map_err(|error| format!("failed to read {}: {error}", path.display()))
        }
        Ok(_) => Err(format!(
            "unsupported transaction target type: {}",
            path.display()
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to inspect {}: {error}", path.display())),
    }
}

/// Atomically restore a regular file only while its bytes still match the state
/// captured immediately before rollback. This deliberately does not follow
/// symlinks or replace directories.
pub(crate) fn write_bytes_if_unchanged(
    path: &Path,
    expected: Option<(&[u8], Option<u32>)>,
    content: &[u8],
    mode: Option<u32>,
) -> Result<(), String> {
    if !file_bytes_match(read_optional_bytes(path)?.as_ref(), expected) {
        return Err(format!(
            "refusing to restore {}: file changed before rollback",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");
    let temp = parent.join(format!(
        ".{}.mux-rollback-{}-{}-{}.tmp",
        file_name,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("failed to create {}: {error}", temp.display()))?;
        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(mode))
                .map_err(|error| error.to_string())?;
        }
        #[cfg(not(unix))]
        let _ = mode;
        file.write_all(content).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        if !file_bytes_match(read_optional_bytes(path)?.as_ref(), expected) {
            return Err(format!(
                "refusing to restore {}: file changed during rollback",
                path.display()
            ));
        }
        if expected.is_none() {
            // A hard link publishes a fully-written same-filesystem temp without
            // replacing a path that appeared after the final CAS check.
            fs::hard_link(&temp, path).map_err(|error| {
                format!(
                    "refusing to create {} during rollback: {error}",
                    path.display()
                )
            })?;
            fs::remove_file(&temp).map_err(|error| error.to_string())
        } else {
            fs::rename(&temp, path).map_err(|error| {
                format!("failed to atomically restore {}: {error}", path.display())
            })
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Remove a rollback-created regular file only if its bytes still match the
/// state captured after the failed operation.
pub(crate) fn remove_bytes_if_unchanged(
    path: &Path,
    expected: &[u8],
    expected_mode: Option<u32>,
) -> Result<(), String> {
    let expected = Some((expected, expected_mode));
    if !file_bytes_match(read_optional_bytes(path)?.as_ref(), expected) {
        return Err(format!(
            "refusing to remove {} during rollback: file changed",
            path.display()
        ));
    }
    // Recheck immediately before unlinking. MUX writers are serialized by the
    // settings lock; this second comparison protects against ordinary external
    // edits during rollback preparation.
    if !file_bytes_match(read_optional_bytes(path)?.as_ref(), expected) {
        return Err(format!(
            "refusing to remove {} during rollback: file changed",
            path.display()
        ));
    }
    fs::remove_file(path).map_err(|error| format!("failed to remove {}: {error}", path.display()))
}

fn file_bytes_match(actual: Option<&FileBytes>, expected: Option<(&[u8], Option<u32>)>) -> bool {
    match (actual, expected) {
        (None, None) => true,
        (Some(actual), Some((bytes, mode))) => actual.bytes == bytes && actual.mode == mode,
        _ => false,
    }
}

fn read_optional_symlink(path: &Path) -> Result<Option<PathBuf>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::read_link(path)
            .map(Some)
            .map_err(|error| format!("failed to read symlink {}: {error}", path.display())),
        Ok(_) => Err(format!(
            "refusing to treat non-symlink {} as a transaction link",
            path.display()
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to inspect {}: {error}", path.display())),
    }
}

pub(crate) fn remove_symlink_if_unchanged(path: &Path, expected: &Path) -> Result<(), String> {
    if read_optional_symlink(path)?.as_deref() != Some(expected) {
        return Err(format!(
            "refusing to remove symlink {} during rollback: link changed",
            path.display()
        ));
    }
    if read_optional_symlink(path)?.as_deref() != Some(expected) {
        return Err(format!(
            "refusing to remove symlink {} during rollback: link changed",
            path.display()
        ));
    }
    fs::remove_file(path)
        .map_err(|error| format!("failed to remove symlink {}: {error}", path.display()))
}

pub(crate) fn write_symlink_if_unchanged(
    path: &Path,
    expected: Option<&Path>,
    target: &Path,
) -> Result<(), String> {
    if read_optional_symlink(path)?.as_deref() != expected {
        return Err(format!(
            "refusing to restore symlink {}: link changed before rollback",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    if expected.is_none() {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, path)
            .map_err(|error| format!("failed to restore symlink {}: {error}", path.display()));
        #[cfg(windows)]
        return std::os::windows::fs::symlink_dir(target, path)
            .map_err(|error| format!("failed to restore symlink {}: {error}", path.display()));
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("link");
    let temp = parent.join(format!(
        ".{}.mux-rollback-link-{}-{}-{}.tmp",
        file_name,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, &temp).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, &temp).map_err(|error| error.to_string())?;
    let result = (|| {
        if read_optional_symlink(path)?.as_deref() != expected {
            return Err(format!(
                "refusing to restore symlink {}: link changed during rollback",
                path.display()
            ));
        }
        fs::rename(&temp, path)
            .map_err(|error| format!("failed to restore symlink {}: {error}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_if_unchanged_impl(
    path: &Path,
    expected: Option<&str>,
    content: &str,
    private: bool,
) -> Result<(), String> {
    if read_optional(path)?.as_deref() != expected {
        return Err(format!(
            "refusing to modify {}: file changed while MUX was preparing the update",
            path.display()
        ));
    }

    let destination = resolve_destination(path)?;
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let permissions = fs::metadata(&destination)
        .ok()
        .map(|metadata| metadata.permissions());
    #[cfg(unix)]
    let permissions = if private {
        use std::os::unix::fs::PermissionsExt;
        Some(fs::Permissions::from_mode(0o600))
    } else {
        permissions
    };
    #[cfg(not(unix))]
    let _ = private;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");
    let temp = parent.join(format!(
        ".{}.mux-{}-{}-{}.tmp",
        file_name,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("failed to create {}: {}", temp.display(), error))?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)
                .map_err(|error| error.to_string())?;
        }
        file.write_all(content.as_bytes())
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;

        if read_optional(path)?.as_deref() != expected {
            return Err(format!(
                "refusing to modify {}: file changed while MUX was preparing the update",
                path.display()
            ));
        }
        if resolve_destination(path)? != destination {
            return Err(format!(
                "refusing to modify {}: symlink target changed while MUX was preparing the update",
                path.display()
            ));
        }
        fs::rename(&temp, &destination).map_err(|error| {
            format!(
                "failed to atomically replace {}: {}",
                destination.display(),
                error
            )
        })?;
        record_transaction_file(&destination, content.as_bytes())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub(crate) fn write_if_unchanged_with_settings_lock(
    path: &Path,
    expected: Option<&str>,
    content: &str,
) -> Result<(), String> {
    let _lock = acquire_settings_lock(path)?;
    write_if_unchanged(path, expected, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mux-safe-write-{}-{}", name, std::process::id()))
    }

    #[test]
    fn refuses_to_replace_a_concurrent_edit() {
        let path = temp_file("concurrent");
        fs::write(&path, "newer").unwrap();
        let result = write_if_unchanged(&path, Some("older"), "mux");
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "newer");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn creates_and_replaces_files() {
        let path = temp_file("replace");
        let _ = fs::remove_file(&path);
        write_if_unchanged(&path, None, "first").unwrap();
        write_if_unchanged(&path, Some("first"), "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn settings_lock_is_released_after_write() {
        let path = temp_file("lock");
        let lock_path = append_suffix(&path, ".lockfile");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&lock_path);
        write_if_unchanged_with_settings_lock(&path, None, "locked").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "locked");
        assert!(lock_path.is_file());
        let reacquired = acquire_settings_lock(&path).unwrap();
        drop(reacquired);
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(lock_path);
    }

    #[test]
    fn settings_lock_is_reentrant_on_the_owning_thread() {
        let path = temp_file("reentrant-lock");
        let lock_path = append_suffix(&path, ".lockfile");
        let _ = fs::remove_file(&lock_path);

        let outer = acquire_settings_lock(&path).unwrap();
        assert!(lock_path.exists());
        {
            let _inner = acquire_settings_lock(&path).unwrap();
            assert!(lock_path.exists());
        }
        assert!(
            lock_path.exists(),
            "dropping a nested guard must retain the outer lock"
        );
        drop(outer);
        assert!(lock_path.is_file());
        let reacquired = acquire_settings_lock(&path).unwrap();
        drop(reacquired);
        let _ = fs::remove_file(lock_path);
    }

    #[test]
    fn settings_lock_remains_held_when_reentrant_guards_drop_out_of_order() {
        let path = temp_file("reentrant-lock-out-of-order");
        let lock_path = append_suffix(&path, ".lockfile");
        let _ = fs::remove_file(&lock_path);

        let outer = acquire_settings_lock(&path).unwrap();
        let inner = acquire_settings_lock(&path).unwrap();
        drop(outer);

        let contender = open_settings_lock_file(&lock_path).unwrap();
        let error = contender.try_lock_exclusive().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::WouldBlock);

        drop(inner);
        contender.try_lock_exclusive().unwrap();
        FileExt::unlock(&contender).unwrap();
        let _ = fs::remove_file(lock_path);
    }

    #[test]
    fn settings_read_lock_does_not_initialize_an_empty_home() {
        let home = crate::testenv::TestHome::new("read-lock-empty");
        let path = crate::paths::settings_file();
        assert!(!home.home.join(".mux").exists());

        let guard = acquire_settings_read_lock_if_initialized(&path).unwrap();

        assert!(guard.is_none());
        assert!(
            !home.home.join(".mux").exists(),
            "a pure read must not create MUX storage"
        );
    }

    #[test]
    fn settings_read_lock_excludes_a_cooperating_writer() {
        use std::sync::mpsc;

        let home = crate::testenv::TestHome::new("read-lock-writer");
        let path = crate::paths::settings_file();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{}").unwrap();
        let read_guard = acquire_settings_read_lock_if_initialized(&path)
            .unwrap()
            .expect("existing settings should initialize a shared lock");
        let writer_path = path.clone();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            let guard = acquire_settings_lock(&writer_path).unwrap();
            acquired_tx.send(()).unwrap();
            drop(guard);
        });

        assert!(
            acquired_rx
                .recv_timeout(Duration::from_millis(150))
                .is_err(),
            "an exclusive writer acquired while the workspace read lock was held"
        );
        drop(read_guard);
        acquired_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        writer.join().unwrap();

        assert!(home.home.join(".mux/settings.json.lockfile").is_file());
    }

    #[test]
    fn settings_read_lock_reenters_an_owned_exclusive_lock() {
        let _home = crate::testenv::TestHome::new("read-lock-reentrant");
        let path = crate::paths::settings_file();
        let exclusive = acquire_settings_lock(&path).unwrap();

        let read = acquire_settings_read_lock_if_initialized(&path)
            .unwrap()
            .expect("owned exclusive lock should be reusable for a read");

        drop(read);
        drop(exclusive);
        let reacquired = acquire_settings_lock(&path).unwrap();
        drop(reacquired);
    }

    #[test]
    fn rollback_bytes_use_the_captured_state_as_a_cas_precondition() {
        let path = temp_file("rollback-bytes-cas");
        let _ = fs::remove_file(&path);
        fs::write(&path, b"mux-partial").unwrap();
        fs::write(&path, b"external-edit").unwrap();

        let mode = file_mode(&path).unwrap();
        let error =
            write_bytes_if_unchanged(&path, Some((b"mux-partial", mode)), b"original", None)
                .unwrap_err();
        assert!(error.contains("changed before rollback"), "{error}");
        assert_eq!(fs::read(&path).unwrap(), b"external-edit");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rollback_bytes_restore_atomically_when_the_cas_matches() {
        let path = temp_file("rollback-bytes-success");
        let _ = fs::remove_file(&path);
        fs::write(&path, b"mux-partial").unwrap();

        let mode = file_mode(&path).unwrap();
        write_bytes_if_unchanged(&path, Some((b"mux-partial", mode)), b"original", None).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"original");
        let _ = fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn preserves_permissions_and_symlink() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let target = temp_file("symlink-target");
        let link = temp_file("symlink-link");
        let _ = fs::remove_file(&target);
        let _ = fs::remove_file(&link);
        fs::write(&target, "old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, &link).unwrap();

        write_if_unchanged(&link, Some("old"), "new").unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = fs::remove_file(link);
        let _ = fs::remove_file(target);
    }
}
