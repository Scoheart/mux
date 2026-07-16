use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);
const SETTINGS_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const SETTINGS_LOCK_POLL: Duration = Duration::from_millis(25);

pub(crate) struct SettingsLock {
    lock_dir: PathBuf,
    owner_file: PathBuf,
}

impl Drop for SettingsLock {
    fn drop(&mut self) {
        if self.owner_file.exists() {
            let _ = fs::remove_file(&self.owner_file);
            let _ = fs::remove_dir(&self.lock_dir);
        }
    }
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

/// Coordinate cooperating writers with a populated lock directory next to the
/// shared settings file. Never reclaim a lock we do not own.
pub(crate) fn acquire_settings_lock(path: &Path) -> Result<SettingsLock, String> {
    let lock_dir = append_suffix(path, ".lock");
    let parent = lock_dir
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let started = Instant::now();
    loop {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let token = format!(
            "{}.{}.{}",
            std::process::id(),
            nonce,
            NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
        );
        let staging = append_suffix(&lock_dir, &format!(".tmp.{token}"));
        fs::create_dir(&staging)
            .map_err(|error| format!("failed to create lock staging directory: {error}"))?;
        let staged_owner = staging.join(format!("owner.{token}"));
        if let Err(error) = fs::write(&staged_owner, &token) {
            let _ = fs::remove_dir_all(&staging);
            return Err(error.to_string());
        }

        match fs::rename(&staging, &lock_dir) {
            Ok(()) => {
                return Ok(SettingsLock {
                    owner_file: lock_dir.join(format!("owner.{token}")),
                    lock_dir,
                });
            }
            Err(_) if lock_dir.exists() => {
                let _ = fs::remove_dir_all(&staging);
                if started.elapsed() >= SETTINGS_LOCK_TIMEOUT {
                    return Err(format!(
                        "refusing to modify {}: timed out waiting for the settings lock",
                        path.display()
                    ));
                }
                thread::sleep(SETTINGS_LOCK_POLL);
            }
            Err(error) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(format!(
                    "failed to acquire settings lock for {}: {}",
                    path.display(),
                    error
                ));
            }
        }
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
    fs::remove_file(path).map_err(|error| format!("failed to remove {}: {}", path.display(), error))
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
        })
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
        let lock_path = append_suffix(&path, ".lock");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&lock_path);
        write_if_unchanged_with_settings_lock(&path, None, "locked").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "locked");
        assert!(!lock_path.exists());
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
