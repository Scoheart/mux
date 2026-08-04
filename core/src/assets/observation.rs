//! Filesystem inputs whose changes may alter desired/observed projections.

use crate::resources::mcp::scanner::expand_tilde;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ObservationDomain {
    Mcp,
    Model,
    Skill,
    Central,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObservationWatchTarget {
    pub domain: ObservationDomain,
    pub path: PathBuf,
    pub recursive: bool,
}

/// Return concrete source-of-truth paths without creating any directories.
/// Hosts may watch the nearest existing ancestor so atomic replacement and a
/// file appearing for the first time are both observable.
pub fn observation_watch_targets() -> Vec<ObservationWatchTarget> {
    let mut targets = BTreeSet::new();
    targets.insert(ObservationWatchTarget {
        domain: ObservationDomain::Central,
        path: crate::paths::settings_file(),
        recursive: false,
    });
    targets.insert(ObservationWatchTarget {
        domain: ObservationDomain::Skill,
        path: crate::paths::mux_dir().join("skills"),
        recursive: true,
    });
    for definition in crate::agents::load_agents().into_values() {
        if let Some(path) = definition.global {
            targets.insert(ObservationWatchTarget {
                domain: ObservationDomain::Mcp,
                path: expand_tilde(&path),
                recursive: false,
            });
        }
    }
    for agent in crate::resources::model::list_agents() {
        if agent.mode != "managed" {
            continue;
        }
        for path in agent.config_paths {
            targets.insert(ObservationWatchTarget {
                domain: ObservationDomain::Model,
                path: expand_tilde(&path),
                recursive: false,
            });
        }
    }
    if let Ok(capabilities) = crate::resources::skill::list_skill_agent_capabilities() {
        for capability in capabilities {
            targets.insert(ObservationWatchTarget {
                domain: ObservationDomain::Skill,
                path: expand_tilde(&capability.global_dir),
                recursive: true,
            });
        }
    }
    targets.into_iter().collect()
}
