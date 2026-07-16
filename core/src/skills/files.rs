use super::{
    io_error, normalized_error_path, parse_manifest, FileChangeKind, SkillContentKind, SkillError,
    SkillFile, SkillFileChange, SkillFileKind, ValidatedSkill,
};
use sha2::{Digest, Sha256};
use similar::TextDiff;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub const MAX_DOWNLOAD_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_SKILL_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_SINGLE_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRIES: u64 = 10_000;
pub const MAX_DIFF_INPUT_BYTES: u64 = 1024 * 1024;
pub const MAX_DIFF_OUTPUT_BYTES: usize = 256 * 1024;
pub const MAX_PLAN_DIFF_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    Directory,
    File,
    Symlink,
}

#[derive(Debug, Clone)]
struct TreeNode {
    path: String,
    full_path: PathBuf,
    kind: NodeKind,
    size: u64,
    executable: bool,
    link_target: Option<String>,
    sha256: Option<String>,
    mode: u32,
}

impl TreeNode {
    fn as_skill_file(&self) -> Option<SkillFile> {
        let kind = match self.kind {
            NodeKind::Directory => return None,
            NodeKind::File => SkillFileKind::File,
            NodeKind::Symlink => SkillFileKind::Symlink,
        };
        Some(SkillFile {
            path: self.path.clone(),
            kind,
            size: self.size,
            executable: self.executable,
            link_target: self.link_target.clone(),
            sha256: self.sha256.clone().unwrap_or_default(),
        })
    }
}

struct TreeSnapshot {
    nodes: Vec<TreeNode>,
    total_bytes: u64,
}

impl TreeSnapshot {
    fn files(&self) -> Vec<SkillFile> {
        self.nodes
            .iter()
            .filter_map(TreeNode::as_skill_file)
            .collect()
    }

    fn file_nodes(&self) -> impl Iterator<Item = &TreeNode> {
        self.nodes
            .iter()
            .filter(|node| node.kind != NodeKind::Directory)
    }
}

struct WalkState {
    nodes: Vec<TreeNode>,
    entries: u64,
    total_bytes: u64,
}

pub fn inspect_tree(root: &Path) -> Result<Vec<SkillFile>, SkillError> {
    Ok(load_tree(root)?.files())
}

pub fn hash_tree(root: &Path) -> Result<String, SkillError> {
    let snapshot = load_tree(root)?;
    hash_snapshot(&snapshot)
}

pub fn copy_tree_secure(source: &Path, destination: &Path) -> Result<(), SkillError> {
    let snapshot = load_tree(source)?;
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(SkillError::Conflict {
                message: "copy destination already exists".into(),
                path: normalized_error_path(destination),
            });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(destination, error)),
    }

    let source_root = fs::canonicalize(source).map_err(|error| io_error(source, error))?;
    let destination_absolute = resolve_destination(destination)?;
    if destination_absolute.starts_with(&source_root) {
        return Err(SkillError::UnsafePath {
            message: "copy destination must not be inside the source tree".into(),
            path: normalized_error_path(destination),
        });
    }

    if let Some(parent) = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    }
    fs::create_dir(destination).map_err(|error| io_error(destination, error))?;
    set_directory_private(destination)?;

    let copy_result = (|| {
        for node in &snapshot.nodes {
            let target = destination.join(path_from_normalized(&node.path));
            match node.kind {
                NodeKind::Directory => {
                    fs::create_dir(&target).map_err(|error| io_error(&target, error))?;
                    set_directory_private(&target)?;
                }
                NodeKind::File => copy_regular_file(node, &target)?,
                NodeKind::Symlink => create_relative_symlink(node, &target)?,
            }
        }
        Ok(())
    })();

    if copy_result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    copy_result
}

pub fn diff_trees(before: Option<&Path>, after: &Path) -> Result<Vec<SkillFileChange>, SkillError> {
    let before_snapshot = before.map(load_tree).transpose()?;
    let after_snapshot = load_tree(after)?;
    let before_nodes = before_snapshot
        .as_ref()
        .map(file_node_map)
        .unwrap_or_default();
    let after_nodes = file_node_map(&after_snapshot);
    let paths: BTreeSet<&str> = before_nodes
        .keys()
        .chain(after_nodes.keys())
        .copied()
        .collect();
    let mut remaining_diff_bytes = MAX_PLAN_DIFF_BYTES;
    let mut changes = Vec::new();

    for path in paths {
        let before_node = before_nodes.get(path).copied();
        let after_node = after_nodes.get(path).copied();
        let kind = match (before_node, after_node) {
            (None, Some(_)) => FileChangeKind::Added,
            (Some(_), None) => FileChangeKind::Removed,
            (Some(old), Some(new)) if old.kind != new.kind => FileChangeKind::LinkChanged,
            (Some(old), Some(new)) if old.kind == NodeKind::Symlink && old.sha256 != new.sha256 => {
                FileChangeKind::LinkChanged
            }
            (Some(old), Some(new)) if old.sha256 != new.sha256 => FileChangeKind::Modified,
            (Some(old), Some(new)) if old.executable != new.executable => {
                FileChangeKind::ModeChanged
            }
            (Some(_), Some(_)) => continue,
            (None, None) => continue,
        };

        let (unified_diff, diff_truncated) = if kind == FileChangeKind::Modified {
            build_text_diff(
                path,
                before_node.expect("modified paths have a before node"),
                after_node.expect("modified paths have an after node"),
                &mut remaining_diff_bytes,
            )?
        } else {
            (None, false)
        };
        changes.push(SkillFileChange {
            path: path.to_string(),
            kind,
            before_hash: before_node.and_then(|node| node.sha256.clone()),
            after_hash: after_node.and_then(|node| node.sha256.clone()),
            unified_diff,
            diff_truncated,
        });
    }
    Ok(changes)
}

pub fn validate_candidate(root: &Path) -> Result<ValidatedSkill, SkillError> {
    let snapshot = load_tree(root)?;
    let manifest_node = snapshot
        .file_nodes()
        .find(|node| node.path == "SKILL.md" && node.kind == NodeKind::File)
        .ok_or_else(|| SkillError::InvalidManifest {
            message: "candidate must contain a regular SKILL.md file".into(),
            path: normalized_error_path(root),
        })?;
    let manifest_bytes = read_file_bounded(&manifest_node.full_path, MAX_SINGLE_FILE_BYTES)?;
    let manifest_text =
        std::str::from_utf8(&manifest_bytes).map_err(|_| SkillError::InvalidManifest {
            message: "SKILL.md must be valid UTF-8".into(),
            path: normalized_error_path(root),
        })?;
    let manifest = parse_manifest(root, manifest_text)?;
    let files = snapshot.files();
    let content_kind = classify_content(&files);
    let content_hash = hash_snapshot(&snapshot)?;
    Ok(ValidatedSkill {
        manifest,
        content_kind,
        files,
        content_hash,
        total_bytes: snapshot.total_bytes,
    })
}

fn load_tree(root: &Path) -> Result<TreeSnapshot, SkillError> {
    let metadata = fs::symlink_metadata(root).map_err(|error| io_error(root, error))?;
    if !metadata.file_type().is_dir() {
        return Err(SkillError::UnsafePath {
            message: "Skill root must be a directory, not a symlink or file".into(),
            path: normalized_error_path(root),
        });
    }
    let canonical_root = fs::canonicalize(root).map_err(|error| io_error(root, error))?;
    let mut state = WalkState {
        nodes: Vec::new(),
        entries: 0,
        total_bytes: 0,
    };
    walk_directory(root, root, &canonical_root, &mut state)?;
    state
        .nodes
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(TreeSnapshot {
        nodes: state.nodes,
        total_bytes: state.total_bytes,
    })
}

fn walk_directory(
    root: &Path,
    directory: &Path,
    canonical_root: &Path,
    state: &mut WalkState,
) -> Result<(), SkillError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_error(directory, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(directory, error))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        state.entries = state.entries.saturating_add(1);
        if state.entries > MAX_ARCHIVE_ENTRIES {
            return Err(SkillError::LimitExceeded {
                limit: "entries".into(),
                actual: state.entries,
                allowed: MAX_ARCHIVE_ENTRIES,
            });
        }
        let relative = normalized_relative(root, &path)?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
        let file_type = metadata.file_type();
        if file_type.is_dir() {
            state.nodes.push(TreeNode {
                path: relative,
                full_path: path.clone(),
                kind: NodeKind::Directory,
                size: 0,
                executable: false,
                link_target: None,
                sha256: None,
                mode: unix_mode(&metadata),
            });
            walk_directory(root, &path, canonical_root, state)?;
        } else if file_type.is_file() {
            if has_multiple_hard_links(&metadata) {
                return Err(SkillError::UnsafePath {
                    message: "hard-linked files are not allowed in Skill trees".into(),
                    path: normalized_error_path(&path),
                });
            }
            if metadata.len() > MAX_SINGLE_FILE_BYTES {
                return Err(SkillError::LimitExceeded {
                    limit: "single_file".into(),
                    actual: metadata.len(),
                    allowed: MAX_SINGLE_FILE_BYTES,
                });
            }
            enforce_total_limit(state.total_bytes, metadata.len())?;
            let (actual_size, sha256) = hash_regular_file(&path)?;
            if actual_size != metadata.len() {
                return Err(SkillError::Conflict {
                    message: "a Skill file changed during inspection".into(),
                    path: normalized_error_path(&path),
                });
            }
            state.total_bytes += actual_size;
            let mode = unix_mode(&metadata);
            state.nodes.push(TreeNode {
                path: relative,
                full_path: path,
                kind: NodeKind::File,
                size: actual_size,
                executable: mode & 0o111 != 0,
                link_target: None,
                sha256: Some(sha256),
                mode,
            });
        } else if file_type.is_symlink() {
            let target = fs::read_link(&path).map_err(|error| io_error(&path, error))?;
            if target.is_absolute() {
                return Err(SkillError::UnsafePath {
                    message: "Skill symlinks must use relative targets".into(),
                    path: normalized_error_path(&path),
                });
            }
            let target_text = target.to_str().ok_or_else(|| SkillError::UnsafePath {
                message: "Skill symlink targets must be valid UTF-8".into(),
                path: normalized_error_path(&path),
            })?;
            let resolved =
                fs::canonicalize(path.parent().unwrap_or(root).join(&target)).map_err(|_| {
                    SkillError::UnsafePath {
                        message: "Skill symlink target cannot be resolved".into(),
                        path: normalized_error_path(&path),
                    }
                })?;
            if !resolved.starts_with(canonical_root) {
                return Err(SkillError::UnsafePath {
                    message: "Skill symlink target escapes the Skill root".into(),
                    path: normalized_error_path(&path),
                });
            }
            let target_bytes = target_text.as_bytes();
            enforce_total_limit(state.total_bytes, target_bytes.len() as u64)?;
            state.total_bytes += target_bytes.len() as u64;
            state.nodes.push(TreeNode {
                path: relative,
                full_path: path,
                kind: NodeKind::Symlink,
                size: target_bytes.len() as u64,
                executable: false,
                link_target: Some(target_text.to_string()),
                sha256: Some(hex::encode(Sha256::digest(target_bytes))),
                mode: 0,
            });
        } else {
            return Err(SkillError::UnsafePath {
                message: "special files are not allowed in Skill trees".into(),
                path: normalized_error_path(&path),
            });
        }
    }
    Ok(())
}

fn normalized_relative(root: &Path, path: &Path) -> Result<String, SkillError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| SkillError::UnsafePath {
            message: "Skill entry is outside the Skill root".into(),
            path: normalized_error_path(path),
        })?;
    if relative.is_absolute() {
        return Err(SkillError::UnsafePath {
            message: "Skill entry paths must be relative".into(),
            path: normalized_error_path(path),
        });
    }
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                parts.push(value.to_str().ok_or_else(|| SkillError::UnsafePath {
                    message: "Skill entry paths must be valid UTF-8".into(),
                    path: normalized_error_path(path),
                })?);
            }
            _ => {
                return Err(SkillError::UnsafePath {
                    message: "Skill entry path contains an unsafe component".into(),
                    path: normalized_error_path(path),
                });
            }
        }
    }
    if parts.is_empty() {
        return Err(SkillError::UnsafePath {
            message: "Skill entry path is empty".into(),
            path: normalized_error_path(path),
        });
    }
    Ok(parts.join("/"))
}

fn hash_regular_file(path: &Path) -> Result<(u64, String), SkillError> {
    let mut file = File::open(path).map_err(|error| io_error(path, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io_error(path, error))?;
        if read == 0 {
            break;
        }
        size = size.saturating_add(read as u64);
        if size > MAX_SINGLE_FILE_BYTES {
            return Err(SkillError::LimitExceeded {
                limit: "single_file".into(),
                actual: size,
                allowed: MAX_SINGLE_FILE_BYTES,
            });
        }
        hasher.update(&buffer[..read]);
    }
    Ok((size, hex::encode(hasher.finalize())))
}

fn hash_snapshot(snapshot: &TreeSnapshot) -> Result<String, SkillError> {
    let mut tree_hash = Sha256::new();
    for node in snapshot.file_nodes() {
        tree_hash.update(match node.kind {
            NodeKind::File => b"file".as_slice(),
            NodeKind::Symlink => b"symlink".as_slice(),
            NodeKind::Directory => unreachable!(),
        });
        tree_hash.update([u8::from(node.executable)]);
        tree_hash.update((node.path.len() as u64).to_be_bytes());
        tree_hash.update(node.path.as_bytes());
        tree_hash.update(node.size.to_be_bytes());
        match node.kind {
            NodeKind::File => {
                let mut file = File::open(&node.full_path)
                    .map_err(|error| io_error(&node.full_path, error))?;
                let copied = std::io::copy(&mut file, &mut HashWriter(&mut tree_hash))
                    .map_err(|error| io_error(&node.full_path, error))?;
                if copied != node.size {
                    return Err(SkillError::Conflict {
                        message: "a Skill file changed during hashing".into(),
                        path: normalized_error_path(&node.full_path),
                    });
                }
            }
            NodeKind::Symlink => {
                tree_hash.update(node.link_target.as_deref().unwrap_or_default().as_bytes())
            }
            NodeKind::Directory => unreachable!(),
        }
    }
    Ok(hex::encode(tree_hash.finalize()))
}

struct HashWriter<'a>(&'a mut Sha256);

impl Write for HashWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn enforce_total_limit(current: u64, added: u64) -> Result<(), SkillError> {
    let actual = current.saturating_add(added);
    if actual > MAX_SKILL_BYTES {
        return Err(SkillError::LimitExceeded {
            limit: "skill".into(),
            actual,
            allowed: MAX_SKILL_BYTES,
        });
    }
    Ok(())
}

fn copy_regular_file(node: &TreeNode, destination: &Path) -> Result<(), SkillError> {
    let source = File::open(&node.full_path).map_err(|error| io_error(&node.full_path, error))?;
    let mut source = source.take(node.size.saturating_add(1));
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| io_error(destination, error))?;
    let copied =
        std::io::copy(&mut source, &mut target).map_err(|error| io_error(destination, error))?;
    target
        .flush()
        .map_err(|error| io_error(destination, error))?;
    if copied != node.size {
        return Err(SkillError::Conflict {
            message: "a Skill file changed during copying".into(),
            path: normalized_error_path(&node.full_path),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(destination, fs::Permissions::from_mode(node.mode & 0o777))
            .map_err(|error| io_error(destination, error))?;
    }
    Ok(())
}

#[cfg(unix)]
fn create_relative_symlink(node: &TreeNode, destination: &Path) -> Result<(), SkillError> {
    let target = node
        .link_target
        .as_deref()
        .ok_or_else(|| SkillError::UnsafePath {
            message: "validated symlink is missing its relative target".into(),
            path: normalized_error_path(&node.full_path),
        })?;
    std::os::unix::fs::symlink(target, destination).map_err(|error| io_error(destination, error))
}

#[cfg(windows)]
fn create_relative_symlink(node: &TreeNode, destination: &Path) -> Result<(), SkillError> {
    let target = node
        .link_target
        .as_deref()
        .ok_or_else(|| SkillError::UnsafePath {
            message: "validated symlink is missing its relative target".into(),
            path: normalized_error_path(&node.full_path),
        })?;
    let resolved = node
        .full_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(target);
    if resolved.is_dir() {
        std::os::windows::fs::symlink_dir(target, destination)
    } else {
        std::os::windows::fs::symlink_file(target, destination)
    }
    .map_err(|error| io_error(destination, error))
}

fn set_directory_private(path: &Path) -> Result<(), SkillError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error(path, error))?;
    }
    Ok(())
}

fn file_node_map(snapshot: &TreeSnapshot) -> BTreeMap<&str, &TreeNode> {
    snapshot
        .file_nodes()
        .map(|node| (node.path.as_str(), node))
        .collect()
}

fn build_text_diff(
    path: &str,
    before: &TreeNode,
    after: &TreeNode,
    remaining_plan_bytes: &mut usize,
) -> Result<(Option<String>, bool), SkillError> {
    if before.kind != NodeKind::File || after.kind != NodeKind::File {
        return Ok((None, false));
    }
    if before.size > MAX_DIFF_INPUT_BYTES || after.size > MAX_DIFF_INPUT_BYTES {
        return Ok((None, true));
    }
    let before_bytes = read_file_bounded(&before.full_path, MAX_DIFF_INPUT_BYTES)?;
    let after_bytes = read_file_bounded(&after.full_path, MAX_DIFF_INPUT_BYTES)?;
    let Ok(before_text) = std::str::from_utf8(&before_bytes) else {
        return Ok((None, false));
    };
    let Ok(after_text) = std::str::from_utf8(&after_bytes) else {
        return Ok((None, false));
    };
    let before_header = format!("a/{path}");
    let after_header = format!("b/{path}");
    let mut output = TextDiff::from_lines(before_text, after_text)
        .unified_diff()
        .header(&before_header, &after_header)
        .to_string();
    let mut truncated = truncate_utf8(&mut output, MAX_DIFF_OUTPUT_BYTES);
    if output.len() > *remaining_plan_bytes {
        truncated |= truncate_utf8(&mut output, *remaining_plan_bytes);
    }
    *remaining_plan_bytes = remaining_plan_bytes.saturating_sub(output.len());
    Ok((Some(output), truncated))
}

fn truncate_utf8(value: &mut String, maximum: usize) -> bool {
    if value.len() <= maximum {
        return false;
    }
    let mut boundary = maximum;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    true
}

fn read_file_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, SkillError> {
    let file = File::open(path).map_err(|error| io_error(path, error))?;
    let mut bytes = Vec::new();
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    if bytes.len() as u64 > maximum {
        return Err(SkillError::LimitExceeded {
            limit: "single_file".into(),
            actual: bytes.len() as u64,
            allowed: maximum,
        });
    }
    Ok(bytes)
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

fn path_from_normalized(path: &str) -> PathBuf {
    path.split('/').collect()
}

fn lexical_absolute(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn resolve_destination(path: &Path) -> Result<PathBuf, SkillError> {
    let absolute = lexical_absolute(path);
    let mut missing = Vec::new();
    let mut cursor = absolute.as_path();
    loop {
        match fs::canonicalize(cursor) {
            Ok(mut resolved) => {
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = cursor.file_name().ok_or_else(|| io_error(cursor, error))?;
                missing.push(name.to_os_string());
                cursor = cursor.parent().ok_or_else(|| SkillError::UnsafePath {
                    message: "copy destination has no resolvable parent".into(),
                    path: normalized_error_path(path),
                })?;
            }
            Err(error) => return Err(io_error(cursor, error)),
        }
    }
}

#[cfg(unix)]
fn unix_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode()
}

#[cfg(not(unix))]
fn unix_mode(_metadata: &fs::Metadata) -> u32 {
    0
}

#[cfg(unix)]
fn has_multiple_hard_links(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() > 1
}

#[cfg(not(unix))]
fn has_multiple_hard_links(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::SkillsPaths;
    use crate::testenv::TestHome;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/skills")
            .join(name)
    }

    fn copy_safe_fixture(destination: &Path) {
        fs::create_dir_all(destination.join("references")).unwrap();
        fs::copy(
            fixture("safe").join("SKILL.md"),
            destination.join("SKILL.md"),
        )
        .unwrap();
        fs::copy(
            fixture("safe").join("references/guide.md"),
            destination.join("references/guide.md"),
        )
        .unwrap();
    }

    #[test]
    fn rejects_escape_symlinks_and_limit_overflow() {
        let th = TestHome::new("skill-file-safety");
        let root = th.home.join("escape");
        fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink("../../outside", root.join("escape")).unwrap();
        assert!(matches!(
            inspect_tree(&root),
            Err(SkillError::UnsafePath { .. })
        ));

        fs::remove_file(root.join("escape")).unwrap();
        fs::write(
            root.join("large.bin"),
            vec![0_u8; MAX_SINGLE_FILE_BYTES as usize + 1],
        )
        .unwrap();
        assert!(matches!(
            inspect_tree(&root),
            Err(SkillError::LimitExceeded {
                limit: ref name,
                ..
            }) if name == "single_file"
        ));
    }

    #[test]
    fn normalized_tree_hash_is_stable_and_content_sensitive() {
        let first = fixture("safe");
        let th = TestHome::new("skill-tree-hash");
        let copy = th.home.join("safe");
        copy_safe_fixture(&copy);
        assert_eq!(hash_tree(&first).unwrap(), hash_tree(&copy).unwrap());
        fs::write(copy.join("references/guide.md"), "changed").unwrap();
        assert_ne!(hash_tree(&first).unwrap(), hash_tree(&copy).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn mux_owned_skill_roots_are_user_only() {
        use std::os::unix::fs::PermissionsExt;
        let _th = TestHome::new("skill-root-permissions");
        let paths = SkillsPaths::from_env().unwrap();
        for path in [
            paths.skills_dir(),
            paths.staging_skills_dir(),
            paths.backups_skills_dir(),
            paths.journals_skills_dir(),
        ] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn secure_copy_preserves_executables_and_safe_relative_symlinks() {
        use std::os::unix::fs::PermissionsExt;

        let th = TestHome::new("skill-secure-copy");
        let source = th.home.join("source");
        let destination = th.home.join("destination");
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(source.join("SKILL.md"), "instructions").unwrap();
        fs::write(source.join("scripts/run.sh"), "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(
            source.join("scripts/run.sh"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        std::os::unix::fs::symlink("scripts/run.sh", source.join("run")).unwrap();

        copy_tree_secure(&source, &destination).unwrap();

        assert_eq!(
            hash_tree(&source).unwrap(),
            hash_tree(&destination).unwrap()
        );
        assert_eq!(
            fs::read_link(destination.join("run")).unwrap(),
            Path::new("scripts/run.sh")
        );
        assert_ne!(
            fs::metadata(destination.join("scripts/run.sh"))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0
        );
        let files = inspect_tree(&destination).unwrap();
        assert!(files.iter().any(|entry| {
            entry.path == "run"
                && entry.kind == SkillFileKind::Symlink
                && entry.link_target.as_deref() == Some("scripts/run.sh")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn secure_copy_rejects_a_destination_inside_source_through_a_symlinked_parent() {
        let th = TestHome::new("skill-copy-alias-destination");
        let real_parent = th.home.join("real");
        let source = real_parent.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "instructions").unwrap();
        std::os::unix::fs::symlink(&real_parent, th.home.join("alias")).unwrap();
        let aliased_source = th.home.join("alias/source");
        let destination = aliased_source.join("nested-copy");

        assert!(matches!(
            copy_tree_secure(&aliased_source, &destination),
            Err(SkillError::UnsafePath { .. })
        ));
        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_hard_links_special_files_and_absolute_symlinks() {
        use std::os::unix::net::UnixListener;

        let th = TestHome::new("skill-rejected-files");
        let root = th.home.join("root");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("original"), "same inode").unwrap();
        fs::hard_link(root.join("original"), root.join("linked")).unwrap();
        assert!(matches!(
            inspect_tree(&root),
            Err(SkillError::UnsafePath { .. })
        ));

        fs::remove_file(root.join("linked")).unwrap();
        fs::remove_file(root.join("original")).unwrap();
        let listener = UnixListener::bind(root.join("socket")).unwrap();
        assert!(matches!(
            inspect_tree(&root),
            Err(SkillError::UnsafePath { .. })
        ));
        drop(listener);
        fs::remove_file(root.join("socket")).unwrap();

        fs::write(root.join("target"), "target").unwrap();
        std::os::unix::fs::symlink(root.join("target"), root.join("absolute")).unwrap();
        assert!(matches!(
            inspect_tree(&root),
            Err(SkillError::UnsafePath { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn diff_classifies_content_mode_link_and_presence_changes() {
        use std::os::unix::fs::PermissionsExt;

        let th = TestHome::new("skill-tree-diff");
        let before = th.home.join("before");
        let after = th.home.join("after");
        fs::create_dir_all(&before).unwrap();
        fs::create_dir_all(&after).unwrap();
        fs::write(before.join("modified.txt"), "before\n").unwrap();
        fs::write(after.join("modified.txt"), "after\n").unwrap();
        fs::write(before.join("mode.txt"), "same\n").unwrap();
        fs::write(after.join("mode.txt"), "same\n").unwrap();
        fs::set_permissions(after.join("mode.txt"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(before.join("first-target"), "one").unwrap();
        fs::write(after.join("first-target"), "one").unwrap();
        fs::write(before.join("second-target"), "two").unwrap();
        fs::write(after.join("second-target"), "two").unwrap();
        std::os::unix::fs::symlink("first-target", before.join("link")).unwrap();
        std::os::unix::fs::symlink("second-target", after.join("link")).unwrap();
        fs::write(before.join("removed.txt"), "removed\n").unwrap();
        fs::write(after.join("added.txt"), "added\n").unwrap();

        let changes = diff_trees(Some(&before), &after).unwrap();
        let kind = |path: &str| {
            changes
                .iter()
                .find(|change| change.path == path)
                .map(|change| change.kind.clone())
                .unwrap()
        };
        assert_eq!(kind("added.txt"), FileChangeKind::Added);
        assert_eq!(kind("link"), FileChangeKind::LinkChanged);
        assert_eq!(kind("mode.txt"), FileChangeKind::ModeChanged);
        assert_eq!(kind("modified.txt"), FileChangeKind::Modified);
        assert_eq!(kind("removed.txt"), FileChangeKind::Removed);
        let modified = changes
            .iter()
            .find(|change| change.path == "modified.txt")
            .unwrap();
        assert!(modified.before_hash.is_some());
        assert!(modified.after_hash.is_some());
        assert!(modified
            .unified_diff
            .as_deref()
            .unwrap()
            .contains("-before"));
        assert!(modified.unified_diff.as_deref().unwrap().contains("+after"));
        assert!(!modified.diff_truncated);
    }

    #[test]
    fn text_diffs_are_utf8_safely_bounded() {
        let th = TestHome::new("skill-diff-bound");
        let before = th.home.join("before");
        let after = th.home.join("after");
        fs::create_dir_all(&before).unwrap();
        fs::create_dir_all(&after).unwrap();
        fs::write(
            before.join("large.txt"),
            format!("{}\n", "旧".repeat(120_000)),
        )
        .unwrap();
        fs::write(
            after.join("large.txt"),
            format!("{}\n", "新".repeat(120_000)),
        )
        .unwrap();

        let changes = diff_trees(Some(&before), &after).unwrap();
        let change = &changes[0];
        let diff = change.unified_diff.as_ref().unwrap();
        assert!(diff.len() <= MAX_DIFF_OUTPUT_BYTES);
        assert!(change.diff_truncated);
        assert!(std::str::from_utf8(diff.as_bytes()).is_ok());
    }

    #[test]
    fn validates_content_kind_priority() {
        let th = TestHome::new("skill-content-kind");
        let root = th.home.join("content-kind");
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("references")).unwrap();
        fs::write(
            root.join("SKILL.md"),
            "---\nname: content-kind\ndescription: Content kind\n---\nbody\n",
        )
        .unwrap();
        fs::write(root.join("assets/icon.bin"), [0_u8, 159, 146, 150]).unwrap();
        fs::write(root.join("references/guide.md"), "guide").unwrap();
        assert_eq!(
            validate_candidate(&root).unwrap().content_kind,
            SkillContentKind::Assets
        );

        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(root.join("scripts/run.txt"), "run").unwrap();
        assert_eq!(
            validate_candidate(&root).unwrap().content_kind,
            SkillContentKind::Automation
        );
    }

    #[test]
    fn rejects_a_relative_mux_root() {
        let _th = TestHome::new("skill-relative-mux-root");
        std::env::set_var("MUX_HOME", "relative-mux-root");
        assert!(matches!(
            SkillsPaths::from_env(),
            Err(SkillError::InvalidSource { .. })
        ));
    }
}
