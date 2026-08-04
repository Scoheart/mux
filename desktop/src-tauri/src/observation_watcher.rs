use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const EVENT_NAME: &str = "asset-observation-changed";

pub fn start(app: AppHandle) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = sender.send(event);
        },
        Config::default(),
    )
    .map_err(|error| error.to_string())?;

    let mut roots = desired_roots(&mux_core::assets::observation_watch_targets());
    // A single inaccessible Agent path must not disable observation for every
    // other capability. Keep every watch that the host can establish.
    roots.retain(|root, mode| watcher.watch(root, *mode).is_ok());

    std::thread::Builder::new()
        .name("mux-observation-watcher".into())
        .spawn(move || {
            // Keep the watcher owned by this thread. Coalesce editor temp-file
            // sequences and MUX's own atomic writes into one projection refresh.
            while receiver.recv().is_ok() {
                while receiver.recv_timeout(Duration::from_millis(250)).is_ok() {}
                // A target may not exist at startup. After its nearest watched
                // ancestor changes, move the watch closer (and switch Skill
                // directories to recursive mode) so later edits stay live.
                // Agent capability paths are themselves mutable central state.
                // Rebuild targets as well as roots so changing a configured
                // path takes effect without restarting MUX.
                let next = desired_roots(&mux_core::assets::observation_watch_targets());
                if next != roots {
                    for root in roots.keys() {
                        let _ = watcher.unwatch(root);
                    }
                    roots.clear();
                    for (root, mode) in next {
                        if watcher.watch(&root, mode).is_ok() {
                            roots.insert(root, mode);
                        }
                    }
                }
                let _ = app.emit(EVENT_NAME, ());
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn desired_roots(
    targets: &[mux_core::assets::ObservationWatchTarget],
) -> BTreeMap<PathBuf, RecursiveMode> {
    let mut roots = BTreeMap::new();
    for target in targets {
        let Some((root, mode)) = watch_root(&target.path, target.recursive) else {
            continue;
        };
        roots
            .entry(root)
            .and_modify(|current| {
                if mode == RecursiveMode::Recursive {
                    *current = RecursiveMode::Recursive;
                }
            })
            .or_insert(mode);
    }
    roots
}

fn watch_root(path: &Path, recursive: bool) -> Option<(PathBuf, RecursiveMode)> {
    if recursive && path.is_dir() {
        return Some((path.to_path_buf(), RecursiveMode::Recursive));
    }
    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    while !current.exists() {
        current = current.parent()?.to_path_buf();
    }
    Some((current, RecursiveMode::NonRecursive))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mux-observation-watcher-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn missing_target_moves_to_the_nearest_existing_parent() {
        let root = temporary_root("nearest-parent");
        let target = root.join("agent/config/settings.json");
        assert_eq!(
            watch_root(&target, false),
            Some((root.clone(), RecursiveMode::NonRecursive))
        );

        let agent = root.join("agent");
        fs::create_dir(&agent).unwrap();
        assert_eq!(
            watch_root(&target, false),
            Some((agent.clone(), RecursiveMode::NonRecursive))
        );

        let config = agent.join("config");
        fs::create_dir(&config).unwrap();
        assert_eq!(
            watch_root(&target, false),
            Some((config, RecursiveMode::NonRecursive))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_skill_directory_is_recursive() {
        let root = temporary_root("recursive-skill");
        let skills = root.join("skills");
        fs::create_dir(&skills).unwrap();
        assert_eq!(
            watch_root(&skills, true),
            Some((skills, RecursiveMode::Recursive))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
