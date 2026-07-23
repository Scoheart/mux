//! Revisioned cross-domain workspace query.

use crate::domain::agents::AgentCapabilityView;
use crate::domain::assets::ConsumptionInventory;
use crate::domain::error::CoreResult;
use crate::domain::types::RegistryEntry;
use crate::resources::model::ModelProfileView;
use crate::resources::skill::SkillsInventory;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::thread;
use std::time::{Duration, Instant};

const WORKSPACE_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const WORKSPACE_LOCK_POLL: Duration = Duration::from_millis(25);

#[derive(Debug, Serialize)]
pub struct WorkspaceAssets {
    pub mcp: Vec<RegistryEntry>,
    pub models: Vec<ModelProfileView>,
    pub skills: SkillsInventory,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceSnapshot {
    /// Content revision of the returned projection. Mutations can use it to
    /// discard stale frontend state without inventing their own cache rules.
    pub revision: String,
    pub agents: Vec<AgentCapabilityView>,
    pub assets: WorkspaceAssets,
    pub relationships: ConsumptionInventory,
}

fn projection() -> CoreResult<(
    Vec<AgentCapabilityView>,
    WorkspaceAssets,
    ConsumptionInventory,
)> {
    let skills = crate::resources::skill::list_inventory().map_err(super::error::from_skill)?;
    let agents = super::agents::list_capabilities_with_skills(&skills.capabilities);
    let relationships = crate::assets::list_consumption_inventory_with_skills(&skills)
        .map_err(super::error::from_legacy)?;
    let assets = WorkspaceAssets {
        mcp: crate::resources::mcp::registry::read_registry(),
        models: crate::resources::model::list_profiles(),
        skills,
    };
    Ok((agents, assets, relationships))
}

fn serialize_projection(
    projection: &(
        Vec<AgentCapabilityView>,
        WorkspaceAssets,
        ConsumptionInventory,
    ),
) -> CoreResult<Vec<u8>> {
    // Convert through Value so maps originating as HashMap (MCP env/headers)
    // have canonical object-key order before equality and revision hashing.
    let canonical = serde_json::to_value(projection).map_err(|error| {
        crate::domain::error::CoreError::new("serialization", error.to_string())
    })?;
    serde_json::to_vec(&canonical)
        .map_err(|error| crate::domain::error::CoreError::new("serialization", error.to_string()))
}

fn wait_for_cooperative_lock(started: Instant) -> CoreResult<()> {
    if started.elapsed() >= WORKSPACE_LOCK_TIMEOUT {
        return Err(crate::domain::error::CoreError::new(
            "snapshot_lock_timeout",
            "timed out waiting for a consistent MUX workspace read",
        ));
    }
    thread::sleep(
        WORKSPACE_LOCK_POLL.min(WORKSPACE_LOCK_TIMEOUT.saturating_sub(started.elapsed())),
    );
    Ok(())
}

pub fn snapshot() -> CoreResult<WorkspaceSnapshot> {
    super::gate::read(|| {
        // Skills transactions and some cross-domain asset transactions acquire
        // their exclusive locks in opposite orders. Acquire both shared locks
        // with non-blocking attempts and never wait while retaining either one;
        // this coordinates cooperating MUX processes without introducing a
        // reader/writer deadlock. External Agent files cannot participate in
        // that protocol, so two identical consecutive canonical projections
        // are still required; continuous external writes fail explicitly.
        let skills_paths = crate::resources::skill::SkillsPaths::resolve_from_env()
            .map_err(super::error::from_skill)?;
        let settings_path = crate::paths::settings_file();
        'attempts: for _ in 0..3 {
            let lock_started = Instant::now();
            let (skills_guard, settings_guard) = 'locks: loop {
                let skills_guard =
                    match crate::resources::skill::try_acquire_skills_read_lock_if_initialized(
                        &skills_paths,
                    )
                    .map_err(super::error::from_skill)?
                    {
                        crate::resources::skill::TrySkillsReadLock::Missing => None,
                        crate::resources::skill::TrySkillsReadLock::Acquired(lock) => Some(lock),
                        crate::resources::skill::TrySkillsReadLock::Contended => {
                            wait_for_cooperative_lock(lock_started)?;
                            continue 'locks;
                        }
                    };

                match crate::safe_write::try_acquire_settings_read_lock_if_initialized(
                    &settings_path,
                )
                .map_err(|error| crate::domain::error::CoreError::new("settings_lock", error))?
                {
                    crate::safe_write::TrySettingsReadLock::Missing => {
                        break 'locks (skills_guard, None);
                    }
                    crate::safe_write::TrySettingsReadLock::Acquired(lock) => {
                        break 'locks (skills_guard, Some(lock));
                    }
                    crate::safe_write::TrySettingsReadLock::Contended => {
                        // Drop the Skills guard before waiting. An asset
                        // transaction may already own settings and be waiting
                        // for the exclusive Skills lock.
                        drop(skills_guard);
                        wait_for_cooperative_lock(lock_started)?;
                        continue 'locks;
                    }
                }
            };

            let first = projection()?;
            let first_content = serialize_projection(&first)?;
            let second = projection()?;
            let second_content = serialize_projection(&second)?;
            if first_content != second_content {
                continue;
            }

            // A pristine HOME has no lock files and reads must not create
            // ~/.mux. If a cooperating writer initialized either domain while
            // this projection ran, restart and acquire both through the
            // deadlock-avoiding try-lock protocol before accepting the result.
            if skills_guard.is_none()
                && crate::resources::skill::skills_lock_is_initialized(&skills_paths)
                    .map_err(super::error::from_skill)?
            {
                continue 'attempts;
            }
            if settings_guard.is_none()
                && crate::safe_write::settings_lock_is_initialized(&settings_path)
                    .map_err(|error| crate::domain::error::CoreError::new("settings_lock", error))?
            {
                continue 'attempts;
            }

            let (agents, assets, relationships) = second;
            return Ok(WorkspaceSnapshot {
                revision: hex::encode(Sha256::digest(second_content)),
                agents,
                assets,
                relationships,
            });
        }
        Err(crate::domain::error::CoreError::new(
            "snapshot_unstable",
            "MUX state changed repeatedly while building the workspace snapshot",
        ))
    })
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn snapshot_of_pristine_home_does_not_create_mux_storage() {
        let home = crate::testenv::TestHome::new("snapshot-pristine");
        let mux_home = home.home.join(".mux");
        assert!(!mux_home.exists());

        super::snapshot().unwrap();

        assert!(
            !mux_home.exists(),
            "a read-only snapshot must not initialize ~/.mux"
        );
    }

    #[test]
    fn snapshot_try_locks_avoid_opposite_writer_order_deadlock() {
        let _home = crate::testenv::TestHome::new("snapshot-lock-order");
        let skills_paths = crate::resources::skill::SkillsPaths::resolve_from_env().unwrap();
        skills_paths.ensure_mux_root().unwrap();
        // Persist the Skills lock first so the snapshot has both initialized
        // domains to coordinate.
        drop(
            crate::resources::skill::acquire_skills_lock(&skills_paths)
                .expect("initialize Skills lock"),
        );

        let settings_path = crate::paths::settings_file();
        let writer_skills_paths = skills_paths.clone();
        let writer_settings_path = settings_path.clone();
        let (settings_held_tx, settings_held_rx) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            // Cross-domain asset commits can take settings before Skills.
            let settings_guard =
                crate::safe_write::acquire_settings_lock(&writer_settings_path).unwrap();
            settings_held_tx.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(200));
            let skills_guard =
                crate::resources::skill::acquire_skills_lock(&writer_skills_paths).unwrap();
            drop(skills_guard);
            drop(settings_guard);
        });
        settings_held_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        let started = Instant::now();
        super::snapshot().unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "snapshot retained one shared lock while waiting for the other"
        );
        writer.join().unwrap();
    }
}
