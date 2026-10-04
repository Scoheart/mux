//! Live plan ownership. The kernel releases these leases when a process exits.

use fs2::FileExt;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

static OWNERS: LazyLock<Mutex<BTreeMap<PathBuf, File>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

fn lease_path(operation_id: &str) -> PathBuf {
    super::planner::operation_root(operation_id).join("owner.lock")
}

fn options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    options
}

pub(crate) fn retain(operation_id: &str) -> Result<(), String> {
    let path = lease_path(operation_id);
    let file = options().create_new(true).open(&path)
        .map_err(|_| "asset_plan_lease: could not establish plan ownership".to_string())?;
    file.try_lock_exclusive()
        .map_err(|_| "asset_plan_lease: could not lock plan ownership".to_string())?;
    OWNERS.lock().unwrap_or_else(|error| error.into_inner()).insert(path, file);
    Ok(())
}

pub(crate) fn release(operation_id: &str) {
    OWNERS.lock().unwrap_or_else(|error| error.into_inner()).remove(&lease_path(operation_id));
}

/// `None` means a live process owns this plan. Keep the returned guard until
/// cleanup finishes; a PID or an age limit cannot prove an owner has exited.
pub(crate) fn claim_abandoned(operation_id: &str) -> Result<Option<AbandonedPlan>, String> {
    let path = lease_path(operation_id);
    if OWNERS.lock().unwrap_or_else(|error| error.into_inner()).contains_key(&path) {
        return Ok(None);
    }
    let file = match options().open(&path) {
        Ok(file) => file,
        // Plans written before leases existed have no live ownership evidence.
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Some(AbandonedPlan(None))),
        Err(_) => return Err("recovery_required: plan ownership file is unsafe".into()),
    };
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Err("recovery_required: plan ownership file is not a regular file".into());
    }
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(AbandonedPlan(Some(file)))),
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
        Err(_) => Err("recovery_required: plan ownership could not be checked".into()),
    }
}

pub(crate) struct AbandonedPlan(#[allow(dead_code)] Option<File>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_open_cannot_recover_a_live_plan() {
        let _home = crate::testenv::TestHome::new("live-asset-plan-lease");
        let id = uuid::Uuid::new_v4().to_string();
        std::fs::create_dir_all(super::super::planner::operation_root(&id)).unwrap();
        retain(&id).unwrap();
        assert!(claim_abandoned(&id).unwrap().is_none());
        let external = options().open(lease_path(&id)).unwrap();
        assert_eq!(external.try_lock_exclusive().unwrap_err().kind(), ErrorKind::WouldBlock);
        release(&id);
        assert!(claim_abandoned(&id).unwrap().is_some());
    }
}
