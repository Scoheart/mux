use super::{
    hash_tree, inspect_tree, parse_manifest, InventoryState, ManagedSkillRecord, SkillAgentView,
    SkillContentKind, SkillDetail, SkillError, SkillFile, SkillInventoryItem, SkillLocation,
    SkillRiskSummary, SkillSource, SkillTargetView, SkillUpdateState, SkillsInventory, SkillsPaths,
};
use crate::agents::builtin_agents;
use crate::settings::{load_settings_strict, Settings};
use crate::types::{AgentInstallProbe, AgentSkillsCapability};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const MAX_SKILL_MD_DETAIL_BYTES: usize = 1024 * 1024;
const TEST_PROBE_ROOT_ENV: &str = "MUX_TEST_PROBE_ROOT";

#[derive(Debug, Clone)]
struct CatalogSkillAgent {
    id: String,
    name: String,
    capability: AgentSkillsCapability,
    installed: bool,
}

#[derive(Debug, Clone)]
struct PhysicalTarget {
    target_id: String,
    global_dir: String,
    canonical_root: PathBuf,
    primary_agent_ids: BTreeSet<String>,
    affected_agent_ids: BTreeSet<String>,
}

#[derive(Debug)]
struct TargetGraph {
    catalog_agents: BTreeMap<String, CatalogSkillAgent>,
    targets: BTreeMap<String, PhysicalTarget>,
    included_target_ids: BTreeSet<String>,
    agents: Vec<SkillAgentView>,
    target_views: Vec<SkillTargetView>,
}

#[derive(Debug, Clone)]
struct ItemMetadata {
    description: String,
    content_kind: SkillContentKind,
    source: Option<SkillSource>,
    resolved_revision: Option<String>,
    content_hash: Option<String>,
    risk: Option<SkillRiskSummary>,
    update: SkillUpdateState,
    installed_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Clone)]
struct ExternalSummary {
    description: String,
    content_kind: SkillContentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedIdentity {
    Central { name: String },
    Target { target_id: String, name: String },
}

pub fn list_skill_agents() -> Result<Vec<SkillAgentView>, SkillError> {
    let paths = SkillsPaths::from_env()?;
    let settings = strict_settings()?;
    Ok(build_target_graph(&paths, &settings)?.agents)
}

pub fn normalize_agent_selection(agent_ids: &[String]) -> Result<Vec<String>, SkillError> {
    let paths = SkillsPaths::from_env()?;
    let settings = strict_settings()?;
    let graph = build_target_graph(&paths, &settings)?;
    let mut selected_ids = BTreeSet::new();
    let mut retained = BTreeSet::new();

    for agent_id in agent_ids {
        if !valid_name(agent_id) {
            return Err(invalid_source("Agent selection contains an invalid id"));
        }
        if !selected_ids.insert(agent_id.clone()) {
            return Err(invalid_source("Agent selection contains a duplicate id"));
        }
        let Some(agent) = graph.catalog_agents.get(agent_id) else {
            return Err(invalid_source("Agent selection contains an unknown id"));
        };
        if !agent.installed {
            return Err(invalid_source(
                "Agent selection requires current verified installation evidence",
            ));
        }
        retained.insert(agent.capability.target_id.clone());
    }

    loop {
        let mut candidates: Vec<(usize, String)> = retained
            .iter()
            .map(|target_id| {
                let target = &graph.targets[target_id];
                let coverage = target
                    .affected_agent_ids
                    .intersection(&selected_ids)
                    .count();
                (coverage, target_id.clone())
            })
            .collect();
        candidates.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

        let removable = candidates.into_iter().find_map(|(_, target_id)| {
            let selected_primary: Vec<&String> = graph.targets[&target_id]
                .primary_agent_ids
                .intersection(&selected_ids)
                .collect();
            let all_observed_elsewhere = !selected_primary.is_empty()
                && selected_primary.iter().all(|agent_id| {
                    retained.iter().any(|other_id| {
                        other_id != &target_id
                            && graph.targets[other_id]
                                .affected_agent_ids
                                .contains(*agent_id)
                    })
                });
            all_observed_elsewhere.then_some(target_id)
        });

        let Some(target_id) = removable else {
            break;
        };
        retained.remove(&target_id);
    }

    Ok(retained.into_iter().collect())
}

pub fn list_inventory() -> Result<SkillsInventory, SkillError> {
    let paths = SkillsPaths::from_env()?;
    let settings = strict_settings()?;
    let graph = build_target_graph(&paths, &settings)?;
    let mut items = Vec::new();
    let mut metadata_by_name = BTreeMap::new();

    scan_central(&paths, &settings, &graph, &mut items, &mut metadata_by_name)?;
    scan_targets(&paths, &settings, &graph, &metadata_by_name, &mut items)?;
    items.sort_by(|left, right| left.identity.cmp(&right.identity));

    Ok(SkillsInventory {
        items,
        agents: graph.agents,
        targets: graph.target_views,
        recovery_error: None,
    })
}

pub fn get_skill_detail(identity: &str) -> Result<SkillDetail, SkillError> {
    let parsed = parse_identity(identity)?;
    let inventory = list_inventory()?;
    let item = inventory
        .items
        .into_iter()
        .find(|item| item.identity == identity)
        .ok_or_else(|| invalid_source("the requested Skill inventory identity is unavailable"))?;

    if item.states.contains(&InventoryState::Missing)
        || item.states.contains(&InventoryState::BrokenLink)
        || item.states.contains(&InventoryState::ConflictingLink)
    {
        return Ok(SkillDetail {
            item,
            files: Vec::new(),
            skill_md: String::new(),
            skill_md_truncated: false,
        });
    }

    let paths = SkillsPaths::from_env()?;
    let settings = strict_settings()?;
    let graph = build_target_graph(&paths, &settings)?;
    let content_root = detail_content_root(&parsed, &paths, &graph)?;
    let files = inspect_tree(&content_root).map_err(sanitize_detail_error)?;
    let (skill_md, skill_md_truncated) =
        read_skill_md_bounded(&content_root, MAX_SKILL_MD_DETAIL_BYTES)?;

    Ok(SkillDetail {
        item,
        files,
        skill_md,
        skill_md_truncated,
    })
}

fn scan_central(
    paths: &SkillsPaths,
    settings: &Settings,
    graph: &TargetGraph,
    items: &mut Vec<SkillInventoryItem>,
    metadata_by_name: &mut BTreeMap<String, ItemMetadata>,
) -> Result<(), SkillError> {
    let records = settings
        .managed_skills
        .as_ref()
        .cloned()
        .unwrap_or_default();
    let mut entries = BTreeMap::<String, PathBuf>::new();
    let read_dir = fs::read_dir(paths.skills_dir()).map_err(|_| SkillError::Io {
        message: "the central Skills inventory could not be read".into(),
        path: None,
    })?;
    for entry in read_dir {
        let entry = entry.map_err(|_| SkillError::Io {
            message: "the central Skills inventory could not be read".into(),
            path: None,
        })?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if valid_name(&name) {
            entries.insert(name, entry.path());
        }
    }

    for (name, record) in &records {
        let mut states = BTreeSet::from([InventoryState::Managed]);
        let central = paths.central_skill(name);
        match fs::symlink_metadata(&central) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                let actual_hash = hash_tree(&central).map_err(sanitize_detail_error)?;
                if actual_hash != record.content_hash {
                    states.insert(InventoryState::LocallyModified);
                }
            }
            Ok(metadata) if metadata.file_type().is_symlink() => {
                states.insert(InventoryState::ConflictingLink);
            }
            Ok(_) => {
                states.insert(InventoryState::Missing);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                states.insert(InventoryState::Missing);
            }
            Err(_) => {
                return Err(SkillError::Io {
                    message: "a managed Skill could not be inspected".into(),
                    path: None,
                });
            }
        }
        if record.update.available {
            states.insert(InventoryState::UpdateAvailable);
        }
        let metadata = metadata_from_record(record);
        let item = make_item(
            name,
            SkillLocation::Central,
            states,
            metadata.clone(),
            assigned_target_ids(settings, name),
            affected_for_assignments(settings, graph, name),
        );
        metadata_by_name.insert(name.clone(), metadata);
        items.push(item);
        entries.remove(name);
    }

    for (name, path) in entries {
        let mut states = BTreeSet::from([InventoryState::External]);
        let summary = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_dir() => try_external_summary(&path),
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if fs::canonicalize(&path).is_err() {
                    states.insert(InventoryState::BrokenLink);
                } else {
                    states.insert(InventoryState::ConflictingLink);
                }
                None
            }
            Ok(_) => None,
            Err(_) => None,
        };
        let metadata = metadata_from_external(summary);
        let item = make_item(
            &name,
            SkillLocation::Central,
            states,
            metadata.clone(),
            assigned_target_ids(settings, &name),
            affected_for_assignments(settings, graph, &name),
        );
        metadata_by_name.insert(name, metadata);
        items.push(item);
    }
    Ok(())
}

fn scan_targets(
    paths: &SkillsPaths,
    settings: &Settings,
    graph: &TargetGraph,
    metadata_by_name: &BTreeMap<String, ItemMetadata>,
    items: &mut Vec<SkillInventoryItem>,
) -> Result<(), SkillError> {
    let records = settings
        .managed_skills
        .as_ref()
        .cloned()
        .unwrap_or_default();
    let assignments = settings
        .skill_assignments
        .as_ref()
        .cloned()
        .unwrap_or_default();

    for target_id in &graph.included_target_ids {
        let target = &graph.targets[target_id];
        let current_root = paths
            .expand_user(&target.global_dir)
            .ok_or_else(|| invalid_source("a verified target path is no longer valid"))?;
        if canonicalize_deepest(&current_root)? != target.canonical_root {
            return Err(invalid_source(
                "a verified target changed physical location during inventory",
            ));
        }
        let location = SkillLocation::AgentTarget {
            target_id: target_id.clone(),
            global_dir: target.global_dir.clone(),
        };
        let affected: Vec<String> = target.affected_agent_ids.iter().cloned().collect();
        let mut seen = BTreeSet::new();

        if fs::metadata(&target.canonical_root).is_ok_and(|metadata| metadata.is_dir()) {
            let read_dir = fs::read_dir(&target.canonical_root).map_err(|_| SkillError::Io {
                message: "a verified Agent Skills target could not be read".into(),
                path: None,
            })?;
            for entry in read_dir {
                let entry = entry.map_err(|_| SkillError::Io {
                    message: "a verified Agent Skills target could not be read".into(),
                    path: None,
                })?;
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if !valid_name(&name) {
                    continue;
                }
                seen.insert(name.clone());
                let entry_path = entry.path();
                let (states, external_summary) = classify_target_entry(paths, &name, &entry_path)?;
                let metadata = records
                    .get(&name)
                    .map(metadata_from_record)
                    .or_else(|| metadata_by_name.get(&name).cloned())
                    .unwrap_or_else(|| metadata_from_external(external_summary));
                items.push(make_item(
                    &name,
                    location.clone(),
                    states,
                    metadata,
                    assigned_target_ids(settings, &name),
                    affected.clone(),
                ));
            }
        }

        for (name, target_ids) in &assignments {
            if target_ids.contains(target_id) && !seen.contains(name) {
                let metadata = records
                    .get(name)
                    .map(metadata_from_record)
                    .or_else(|| metadata_by_name.get(name).cloned())
                    .unwrap_or_else(|| metadata_from_external(None));
                items.push(make_item(
                    name,
                    location.clone(),
                    BTreeSet::from([InventoryState::Missing]),
                    metadata,
                    assigned_target_ids(settings, name),
                    affected.clone(),
                ));
            }
        }
    }
    Ok(())
}

fn classify_target_entry(
    paths: &SkillsPaths,
    name: &str,
    entry_path: &Path,
) -> Result<(BTreeSet<InventoryState>, Option<ExternalSummary>), SkillError> {
    let metadata = fs::symlink_metadata(entry_path).map_err(|_| SkillError::Io {
        message: "an Agent Skills target entry could not be inspected".into(),
        path: None,
    })?;
    if metadata.file_type().is_symlink() {
        let resolved = match fs::canonicalize(entry_path) {
            Ok(resolved) => resolved,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((BTreeSet::from([InventoryState::BrokenLink]), None));
            }
            Err(_) => {
                return Err(SkillError::Io {
                    message: "an Agent Skills link could not be resolved".into(),
                    path: None,
                });
            }
        };
        let central = paths.central_skill(name);
        let central_resolved = fs::canonicalize(&central).ok();
        if central_resolved.as_ref() == Some(&resolved) {
            return Ok((BTreeSet::from([InventoryState::Assigned]), None));
        }
        return Ok((BTreeSet::from([InventoryState::ConflictingLink]), None));
    }
    if metadata.file_type().is_dir() {
        return Ok((
            BTreeSet::from([InventoryState::External]),
            try_external_summary(entry_path),
        ));
    }
    Ok((BTreeSet::from([InventoryState::External]), None))
}

fn make_item(
    name: &str,
    location: SkillLocation,
    mut states: BTreeSet<InventoryState>,
    metadata: ItemMetadata,
    assigned_target_ids: Vec<String>,
    affected_agent_ids: Vec<String>,
) -> SkillInventoryItem {
    if metadata.update.available {
        states.insert(InventoryState::UpdateAvailable);
    }
    let identity = match &location {
        SkillLocation::Central => format!("central:{name}"),
        SkillLocation::AgentTarget { target_id, .. } => {
            format!("target:{target_id}:{name}")
        }
    };
    SkillInventoryItem {
        identity,
        name: name.into(),
        description: metadata.description,
        content_kind: metadata.content_kind,
        states,
        location,
        source: metadata.source,
        resolved_revision: metadata.resolved_revision,
        content_hash: metadata.content_hash,
        risk: metadata.risk,
        update: metadata.update,
        assigned_target_ids,
        affected_agent_ids,
        installed_at: metadata.installed_at,
        updated_at: metadata.updated_at,
    }
}

fn metadata_from_record(record: &ManagedSkillRecord) -> ItemMetadata {
    ItemMetadata {
        description: record.description.clone(),
        content_kind: record.content_kind.clone(),
        source: Some(record.source.clone()),
        resolved_revision: record.resolved_revision.clone(),
        content_hash: Some(record.content_hash.clone()),
        risk: Some(record.risk.clone()),
        update: record.update.clone(),
        installed_at: Some(record.installed_at.clone()),
        updated_at: Some(record.updated_at.clone()),
    }
}

fn metadata_from_external(summary: Option<ExternalSummary>) -> ItemMetadata {
    ItemMetadata {
        description: summary
            .as_ref()
            .map(|summary| summary.description.clone())
            .unwrap_or_default(),
        content_kind: summary
            .map(|summary| summary.content_kind)
            .unwrap_or(SkillContentKind::Instructions),
        source: None,
        resolved_revision: None,
        content_hash: None,
        risk: None,
        update: SkillUpdateState::default(),
        installed_at: None,
        updated_at: None,
    }
}

fn assigned_target_ids(settings: &Settings, name: &str) -> Vec<String> {
    settings
        .skill_assignments
        .as_ref()
        .and_then(|assignments| assignments.get(name))
        .map(|target_ids| target_ids.iter().cloned().collect())
        .unwrap_or_default()
}

fn affected_for_assignments(settings: &Settings, graph: &TargetGraph, name: &str) -> Vec<String> {
    let mut affected = BTreeSet::new();
    for target_id in assigned_target_ids(settings, name) {
        if let Some(target) = graph.targets.get(&target_id) {
            affected.extend(target.affected_agent_ids.iter().cloned());
        }
    }
    affected.into_iter().collect()
}

fn try_external_summary(root: &Path) -> Option<ExternalSummary> {
    let (skill_md, _) = read_skill_md_bounded(root, MAX_SKILL_MD_DETAIL_BYTES).ok()?;
    let manifest = parse_manifest(root, &skill_md).ok()?;
    let files = inspect_tree(root).ok()?;
    Some(ExternalSummary {
        description: manifest.description,
        content_kind: classify_content(&files),
    })
}

fn classify_content(files: &[SkillFile]) -> SkillContentKind {
    if files
        .iter()
        .any(|file| file.executable || file.path.starts_with("scripts/"))
    {
        SkillContentKind::Automation
    } else if files.iter().any(|file| file.path.starts_with("assets/")) {
        SkillContentKind::Assets
    } else if files
        .iter()
        .any(|file| file.path.starts_with("references/"))
    {
        SkillContentKind::Reference
    } else {
        SkillContentKind::Instructions
    }
}

fn parse_identity(identity: &str) -> Result<ParsedIdentity, SkillError> {
    if let Some(name) = identity.strip_prefix("central:") {
        if valid_name(name) {
            return Ok(ParsedIdentity::Central { name: name.into() });
        }
        return Err(invalid_source("the Skill inventory identity is invalid"));
    }
    if let Some(rest) = identity.strip_prefix("target:") {
        let Some((target_id, name)) = rest.split_once(':') else {
            return Err(invalid_source("the Skill inventory identity is invalid"));
        };
        if valid_name(target_id) && valid_name(name) {
            return Ok(ParsedIdentity::Target {
                target_id: target_id.into(),
                name: name.into(),
            });
        }
    }
    Err(invalid_source("the Skill inventory identity is invalid"))
}

fn detail_content_root(
    identity: &ParsedIdentity,
    paths: &SkillsPaths,
    graph: &TargetGraph,
) -> Result<PathBuf, SkillError> {
    match identity {
        ParsedIdentity::Central { name } => {
            verified_child_directory(&paths.skills_dir(), &paths.central_skill(name))
        }
        ParsedIdentity::Target { target_id, name } => {
            let target = graph
                .targets
                .get(target_id)
                .ok_or_else(|| invalid_source("the Skill target identity is unknown"))?;
            let expanded = paths
                .expand_user(&target.global_dir)
                .ok_or_else(|| invalid_source("the Skill target identity is unavailable"))?;
            if canonicalize_deepest(&expanded)? != target.canonical_root {
                return Err(invalid_source(
                    "the Skill target changed physical location during detail loading",
                ));
            }
            let entry = target.canonical_root.join(name);
            let metadata = fs::symlink_metadata(&entry).map_err(|_| SkillError::Io {
                message: "the Skill target detail is unavailable".into(),
                path: None,
            })?;
            if metadata.file_type().is_symlink() {
                let resolved = fs::canonicalize(&entry).map_err(|_| SkillError::Io {
                    message: "the Skill target detail is unavailable".into(),
                    path: None,
                })?;
                let central = paths.central_skill(name);
                let central_resolved = fs::canonicalize(&central).map_err(|_| SkillError::Io {
                    message: "the central Skill detail is unavailable".into(),
                    path: None,
                })?;
                if resolved != central_resolved {
                    return Err(invalid_source(
                        "the Skill target link is not managed by MUX",
                    ));
                }
                return verified_child_directory(&paths.skills_dir(), &central);
            }
            verified_child_directory(&target.canonical_root, &entry)
        }
    }
}

fn verified_child_directory(parent: &Path, child: &Path) -> Result<PathBuf, SkillError> {
    let metadata = fs::symlink_metadata(child).map_err(|_| SkillError::Io {
        message: "the Skill detail directory is unavailable".into(),
        path: None,
    })?;
    if !metadata.file_type().is_dir() {
        return Err(invalid_source(
            "the Skill detail location is not a regular directory",
        ));
    }
    let parent = fs::canonicalize(parent).map_err(|_| SkillError::Io {
        message: "the Skill detail root is unavailable".into(),
        path: None,
    })?;
    let child = fs::canonicalize(child).map_err(|_| SkillError::Io {
        message: "the Skill detail directory is unavailable".into(),
        path: None,
    })?;
    if child.parent() != Some(parent.as_path()) {
        return Err(invalid_source(
            "the Skill detail directory escaped its verified root",
        ));
    }
    Ok(child)
}

fn sanitize_detail_error(error: SkillError) -> SkillError {
    match error {
        SkillError::LimitExceeded { .. }
        | SkillError::InvalidSource { .. }
        | SkillError::Network { .. }
        | SkillError::PlanStale { .. }
        | SkillError::ConfirmationRequired { .. }
        | SkillError::RecoveryRequired { .. } => error,
        _ => SkillError::Io {
            message: "the Skill tree could not be inspected safely".into(),
            path: None,
        },
    }
}

#[cfg(unix)]
fn read_skill_md_bounded(root: &Path, maximum: usize) -> Result<(String, bool), SkillError> {
    use rustix::fs::{fstat, openat, FileType, Mode, OFlags, CWD};
    use std::fs::File;

    let root_file = File::from(
        openat(
            CWD,
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| SkillError::Io {
            message: "the Skill detail root could not be opened safely".into(),
            path: None,
        })?,
    );
    let root_before = fstat(&root_file).map_err(|_| SkillError::Io {
        message: "the Skill detail root could not be inspected safely".into(),
        path: None,
    })?;
    if FileType::from_raw_mode(root_before.st_mode as _) != FileType::Directory {
        return Err(invalid_source("the Skill detail root is not a directory"));
    }

    let mut file = File::from(
        openat(
            &root_file,
            "SKILL.md",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| SkillError::Io {
            message: "SKILL.md could not be opened safely".into(),
            path: None,
        })?,
    );
    let before = fstat(&file).map_err(|_| SkillError::Io {
        message: "SKILL.md could not be inspected safely".into(),
        path: None,
    })?;
    if FileType::from_raw_mode(before.st_mode as _) != FileType::RegularFile || before.st_nlink != 1
    {
        return Err(invalid_source(
            "SKILL.md must be a regular private tree file",
        ));
    }
    let size = u64::try_from(before.st_size).unwrap_or(u64::MAX);
    let requested = size.min(maximum as u64);
    let mut bytes = Vec::with_capacity(requested as usize);
    (&mut file)
        .take(requested)
        .read_to_end(&mut bytes)
        .map_err(|_| SkillError::Io {
            message: "SKILL.md could not be read safely".into(),
            path: None,
        })?;
    let after = fstat(&file).map_err(|_| SkillError::Io {
        message: "SKILL.md could not be rechecked safely".into(),
        path: None,
    })?;
    let root_after = fstat(&root_file).map_err(|_| SkillError::Io {
        message: "the Skill detail root could not be rechecked safely".into(),
        path: None,
    })?;
    if bytes.len() as u64 != requested
        || before.st_dev != after.st_dev
        || before.st_ino != after.st_ino
        || before.st_nlink != after.st_nlink
        || before.st_size != after.st_size
        || before.st_mode != after.st_mode
        || root_before.st_dev != root_after.st_dev
        || root_before.st_ino != root_after.st_ino
        || root_before.st_mode != root_after.st_mode
    {
        return Err(SkillError::Conflict {
            message: "the Skill detail changed while it was being read".into(),
            path: String::new(),
        });
    }
    decode_skill_md(bytes, size > maximum as u64)
}

#[cfg(not(unix))]
fn read_skill_md_bounded(root: &Path, maximum: usize) -> Result<(String, bool), SkillError> {
    let skill_md = root.join("SKILL.md");
    let metadata = fs::symlink_metadata(&skill_md).map_err(|_| SkillError::Io {
        message: "SKILL.md could not be inspected safely".into(),
        path: None,
    })?;
    if !metadata.file_type().is_file() {
        return Err(invalid_source("SKILL.md must be a regular file"));
    }
    let mut file = fs::File::open(&skill_md).map_err(|_| SkillError::Io {
        message: "SKILL.md could not be opened safely".into(),
        path: None,
    })?;
    let requested = metadata.len().min(maximum as u64);
    let mut bytes = Vec::with_capacity(requested as usize);
    (&mut file)
        .take(requested)
        .read_to_end(&mut bytes)
        .map_err(|_| SkillError::Io {
            message: "SKILL.md could not be read safely".into(),
            path: None,
        })?;
    if bytes.len() as u64 != requested {
        return Err(SkillError::Io {
            message: "SKILL.md changed while it was being read".into(),
            path: None,
        });
    }
    decode_skill_md(bytes, metadata.len() > maximum as u64)
}

fn decode_skill_md(bytes: Vec<u8>, truncated: bool) -> Result<(String, bool), SkillError> {
    match String::from_utf8(bytes) {
        Ok(text) => Ok((text, truncated)),
        Err(error) if truncated && error.utf8_error().error_len().is_none() => {
            let boundary = error.utf8_error().valid_up_to();
            let mut bytes = error.into_bytes();
            bytes.truncate(boundary);
            Ok((
                String::from_utf8(bytes).expect("valid_up_to is a UTF-8 boundary"),
                true,
            ))
        }
        Err(_) => Err(SkillError::InvalidManifest {
            message: "SKILL.md must be valid UTF-8".into(),
            path: String::new(),
        }),
    }
}

fn strict_settings() -> Result<Settings, SkillError> {
    load_settings_strict().map_err(|_| SkillError::Io {
        message: "MUX settings could not be read safely".into(),
        path: None,
    })
}

fn build_target_graph(paths: &SkillsPaths, settings: &Settings) -> Result<TargetGraph, SkillError> {
    validate_skill_settings(settings)?;
    let mut catalog_agents = BTreeMap::new();
    let mut targets = BTreeMap::<String, PhysicalTarget>::new();
    let mut targets_by_path = BTreeMap::<PathBuf, String>::new();

    for (id, definition) in builtin_agents() {
        let Some(capability) = definition.skills else {
            continue;
        };
        if !valid_name(&id)
            || !valid_name(&capability.target_id)
            || capability.evidence != "official"
            || capability.docs.trim().is_empty()
            || capability.verified_at.trim().is_empty()
            || capability.probes.is_empty()
        {
            return Err(invalid_source(
                "the verified Agent Skills catalog is inconsistent",
            ));
        }
        let installed = capability
            .probes
            .iter()
            .any(|probe| probe_installed(probe, paths));
        let agent = CatalogSkillAgent {
            id: id.clone(),
            name: definition.name.unwrap_or_else(|| id.clone()),
            capability: capability.clone(),
            installed,
        };

        register_target(
            paths,
            &mut targets,
            &mut targets_by_path,
            &capability.target_id,
            &capability.global_dir,
            Some(&id),
        )?;
        for alias in &capability.aliases {
            register_target(
                paths,
                &mut targets,
                &mut targets_by_path,
                &alias.target_id,
                &alias.global_dir,
                None,
            )?;
        }
        catalog_agents.insert(id, agent);
    }

    for agent in catalog_agents.values().filter(|agent| agent.installed) {
        let declarations = std::iter::once(agent.capability.target_id.as_str()).chain(
            agent
                .capability
                .aliases
                .iter()
                .map(|alias| alias.target_id.as_str()),
        );
        for target_id in declarations {
            targets
                .get_mut(target_id)
                .expect("verified target was registered")
                .affected_agent_ids
                .insert(agent.id.clone());
        }
    }

    let mut included_target_ids = BTreeSet::new();
    for agent in catalog_agents.values().filter(|agent| agent.installed) {
        included_target_ids.insert(agent.capability.target_id.clone());
        included_target_ids.extend(
            agent
                .capability
                .aliases
                .iter()
                .map(|alias| alias.target_id.clone()),
        );
    }
    for target_id in settings
        .skill_assignments
        .iter()
        .flat_map(|assignments| assignments.values())
        .flatten()
    {
        if !targets.contains_key(target_id) {
            return Err(invalid_source(
                "Skills settings reference an unknown physical target",
            ));
        }
        included_target_ids.insert(target_id.clone());
    }

    let agents = catalog_agents
        .values()
        .filter(|agent| agent.installed)
        .map(|agent| {
            let capability = &agent.capability;
            SkillAgentView {
                id: agent.id.clone(),
                name: agent.name.clone(),
                target_id: capability.target_id.clone(),
                global_dir: capability.global_dir.clone(),
                affected_agent_ids: targets[&capability.target_id]
                    .affected_agent_ids
                    .iter()
                    .cloned()
                    .collect(),
                docs: capability.docs.clone(),
                evidence: capability.evidence.clone(),
                verified_at: capability.verified_at.clone(),
            }
        })
        .collect();
    let target_views = included_target_ids
        .iter()
        .map(|target_id| {
            let target = &targets[target_id];
            SkillTargetView {
                target_id: target.target_id.clone(),
                global_dir: target.global_dir.clone(),
                primary_agent_ids: target.primary_agent_ids.iter().cloned().collect(),
                affected_agent_ids: target.affected_agent_ids.iter().cloned().collect(),
                assignable: target.primary_agent_ids.iter().any(|agent_id| {
                    catalog_agents
                        .get(agent_id)
                        .is_some_and(|agent| agent.installed)
                }),
            }
        })
        .collect();

    Ok(TargetGraph {
        catalog_agents,
        targets,
        included_target_ids,
        agents,
        target_views,
    })
}

fn validate_skill_settings(settings: &Settings) -> Result<(), SkillError> {
    if let Some(records) = &settings.managed_skills {
        for (name, record) in records {
            if !valid_name(name) || record.name != *name {
                return Err(invalid_source(
                    "managed Skills settings contain an invalid identity",
                ));
            }
        }
    }
    if let Some(assignments) = &settings.skill_assignments {
        for (name, target_ids) in assignments {
            if !valid_name(name) || target_ids.iter().any(|target_id| !valid_name(target_id)) {
                return Err(invalid_source(
                    "Skills assignment settings contain an invalid identity",
                ));
            }
        }
    }
    Ok(())
}

fn register_target(
    paths: &SkillsPaths,
    targets: &mut BTreeMap<String, PhysicalTarget>,
    targets_by_path: &mut BTreeMap<PathBuf, String>,
    target_id: &str,
    global_dir: &str,
    primary_agent_id: Option<&str>,
) -> Result<(), SkillError> {
    if !valid_name(target_id) {
        return Err(invalid_source(
            "the verified Agent Skills catalog contains an invalid target id",
        ));
    }
    let expanded = paths.expand_user(global_dir).ok_or_else(|| {
        invalid_source("the verified Agent Skills catalog contains an unsafe target")
    })?;
    if !expanded.is_absolute() {
        return Err(invalid_source(
            "the verified Agent Skills catalog contains a relative target",
        ));
    }
    let canonical_root = canonicalize_deepest(&expanded)?;
    match fs::metadata(&canonical_root) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(invalid_source(
                "a verified Agent Skills target is not a directory",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            return Err(invalid_source(
                "a verified Agent Skills target could not be inspected safely",
            ));
        }
    }

    if let Some(existing) = targets.get(target_id) {
        if existing.canonical_root != canonical_root {
            return Err(invalid_source(
                "one verified target id resolves to multiple physical directories",
            ));
        }
    }
    if let Some(existing_id) = targets_by_path.get(&canonical_root) {
        if existing_id != target_id {
            return Err(invalid_source(
                "multiple verified target ids resolve to one physical directory",
            ));
        }
    }

    let target = targets
        .entry(target_id.into())
        .or_insert_with(|| PhysicalTarget {
            target_id: target_id.into(),
            global_dir: global_dir.into(),
            canonical_root: canonical_root.clone(),
            primary_agent_ids: BTreeSet::new(),
            affected_agent_ids: BTreeSet::new(),
        });
    if let Some(agent_id) = primary_agent_id {
        target.primary_agent_ids.insert(agent_id.into());
    }
    targets_by_path.insert(canonical_root, target_id.into());
    Ok(())
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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if fs::symlink_metadata(cursor).is_ok() {
                    return Err(invalid_source(
                        "a verified target has a broken existing parent",
                    ));
                }
                let Some(name) = cursor.file_name() else {
                    return Err(invalid_source("a verified target has no resolvable parent"));
                };
                missing.push(name.to_os_string());
                cursor = cursor
                    .parent()
                    .ok_or_else(|| invalid_source("a verified target has no resolvable parent"))?;
            }
            Err(_) => {
                return Err(invalid_source(
                    "a verified target could not be resolved safely",
                ));
            }
        }
    }
}

fn lexical_absolute(path: &Path) -> Result<PathBuf, SkillError> {
    if !path.is_absolute() {
        return Err(invalid_source("an inventory path must be absolute"));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str())
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(invalid_source("an inventory path escapes its root"));
                }
            }
        }
    }
    Ok(normalized)
}

fn probe_installed(probe: &AgentInstallProbe, paths: &SkillsPaths) -> bool {
    match probe {
        AgentInstallProbe::Path { path } => paths
            .expand_user(path)
            .map(|path| remap_system_probe_path(path, paths.user_home()))
            .is_some_and(|path| path.exists()),
        AgentInstallProbe::Command { name } => command_exists(name, paths.user_home()),
        AgentInstallProbe::MacBundle { bundle_id } => {
            mac_bundle_exists(bundle_id, paths.user_home())
        }
    }
}

fn command_exists(name: &str, user_home: &Path) -> bool {
    let command = Path::new(name);
    if name.is_empty()
        || command.components().count() != 1
        || !matches!(command.components().next(), Some(Component::Normal(_)))
    {
        return false;
    }

    let mut directories = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    directories.extend([
        user_home.join(".local/bin"),
        user_home.join(".cargo/bin"),
        remap_system_root(Path::new("/opt/homebrew/bin"), user_home),
        remap_system_root(Path::new("/usr/local/bin"), user_home),
    ]);

    directories
        .into_iter()
        .map(|directory| directory.join(name))
        .any(|candidate| is_executable_regular_file(&candidate))
}

fn is_executable_regular_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(target_os = "macos")]
fn mac_bundle_exists(bundle_id: &str, user_home: &Path) -> bool {
    let roots = [
        remap_system_root(Path::new("/Applications"), user_home),
        user_home.join("Applications"),
    ];
    roots.into_iter().any(|root| {
        fs::read_dir(root)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                let bundle = entry.path();
                if bundle.extension() != Some(OsStr::new("app")) {
                    return false;
                }
                let plist_path = bundle.join("Contents/Info.plist");
                let Ok(metadata) = fs::metadata(&plist_path) else {
                    return false;
                };
                if !metadata.is_file() || metadata.len() > MAX_SKILL_MD_DETAIL_BYTES as u64 {
                    return false;
                }
                plist::Value::from_file(plist_path)
                    .ok()
                    .and_then(|value| {
                        value
                            .as_dictionary()
                            .and_then(|dict| dict.get("CFBundleIdentifier"))
                            .and_then(plist::Value::as_string)
                            .map(str::to_owned)
                    })
                    .is_some_and(|found| found == bundle_id)
            })
    })
}

#[cfg(not(target_os = "macos"))]
fn mac_bundle_exists(_bundle_id: &str, _user_home: &Path) -> bool {
    false
}

fn remap_system_probe_path(path: PathBuf, user_home: &Path) -> PathBuf {
    let applications = Path::new("/Applications");
    if let Ok(relative) = path.strip_prefix(applications) {
        return remap_system_root(applications, user_home).join(relative);
    }
    path
}

fn remap_system_root(production: &Path, user_home: &Path) -> PathBuf {
    let Some(root) = guarded_test_probe_root(user_home) else {
        return production.to_path_buf();
    };
    production
        .strip_prefix(Path::new("/"))
        .map(|relative| root.join(relative))
        .unwrap_or_else(|_| production.to_path_buf())
}

fn guarded_test_probe_root(user_home: &Path) -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os(TEST_PROBE_ROOT_ENV)?);
    let mux_home = std::env::var_os("MUX_HOME").map(PathBuf::from)?;
    (root.is_absolute() && root == user_home && mux_home == root.join(".mux")).then_some(root)
}

fn invalid_source(message: &str) -> SkillError {
    SkillError::InvalidSource {
        message: message.into(),
    }
}

fn valid_name(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testenv::TestHome;

    #[test]
    fn guarded_probe_root_remaps_only_a_matching_test_home() {
        let th = TestHome::new("skill-probe-root-guard");
        assert_eq!(
            remap_system_root(Path::new("/opt/homebrew/bin"), &th.home),
            th.home.join("opt/homebrew/bin")
        );

        std::env::set_var(TEST_PROBE_ROOT_ENV, th.home.join("mismatch"));
        assert_eq!(
            remap_system_root(Path::new("/opt/homebrew/bin"), &th.home),
            PathBuf::from("/opt/homebrew/bin")
        );
        assert_eq!(
            remap_system_probe_path(PathBuf::from("/Applications/Cursor.app"), &th.home),
            PathBuf::from("/Applications/Cursor.app")
        );
    }

    #[test]
    fn identity_parser_accepts_only_opaque_catalog_shaped_values() {
        assert_eq!(
            parse_identity("central:safe").unwrap(),
            ParsedIdentity::Central {
                name: "safe".into()
            }
        );
        assert_eq!(
            parse_identity("target:cursor-user:safe").unwrap(),
            ParsedIdentity::Target {
                target_id: "cursor-user".into(),
                name: "safe".into()
            }
        );
        for invalid in [
            "central:../safe",
            "central:/tmp/safe",
            "target:cursor-user:/tmp/safe",
            "target:cursor-user:safe:extra",
        ] {
            assert!(parse_identity(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_bundle_probe_reads_only_disposable_top_level_info_plists() {
        let th = TestHome::new("skill-bundle-probe");
        let contents = th.home.join("Applications/Fixture.app/Contents");
        fs::create_dir_all(&contents).unwrap();
        let mut dictionary = plist::Dictionary::new();
        dictionary.insert(
            "CFBundleIdentifier".into(),
            plist::Value::String("com.example.fixture".into()),
        );
        plist::to_file_xml(
            contents.join("Info.plist"),
            &plist::Value::Dictionary(dictionary),
        )
        .unwrap();

        assert!(mac_bundle_exists("com.example.fixture", &th.home));
        assert!(!mac_bundle_exists("com.example.missing", &th.home));
    }
}
