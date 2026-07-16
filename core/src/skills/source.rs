use super::anchored::{AnchoredFileKind, AnchoredRoot};
use super::{
    capped_message, copy_tree_secure, io_error, validate_candidate, SkillCandidateSummary,
    SkillError, SkillSource, SkillSourceInput, SkillSourceResolution, SkillsPaths,
    MAX_ARCHIVE_BYTES, MAX_ARCHIVE_ENTRIES, MAX_DOWNLOAD_BYTES, MAX_SINGLE_FILE_BYTES,
};
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tar::Archive;
use url::Url;
use uuid::Uuid;

const MAX_SOURCE_BYTES: usize = 4096;
const MAX_REF_PROBES: usize = 16;
const MAX_REDIRECTS: usize = 5;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct GithubEndpoints {
    pub api_base: Url,
    pub archive_base: Url,
    allowed_hosts: BTreeSet<String>,
    allow_http_loopback: bool,
}

impl GithubEndpoints {
    pub fn production() -> Self {
        Self {
            api_base: Url::parse("https://api.github.com/").unwrap(),
            archive_base: Url::parse("https://codeload.github.com/").unwrap(),
            allowed_hosts: ["github.com", "api.github.com", "codeload.github.com"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            allow_http_loopback: false,
        }
    }

    #[doc(hidden)]
    pub fn for_test(api_base: Url, archive_base: Url) -> Self {
        let hosts = [&api_base, &archive_base]
            .into_iter()
            .map(|url| url.host_str().expect("test endpoint host").to_owned())
            .collect::<BTreeSet<_>>();
        assert!(hosts
            .iter()
            .all(|host| matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1")));
        Self {
            api_base,
            archive_base,
            allowed_hosts: hosts,
            allow_http_loopback: true,
        }
    }
}

pub fn resolve_source(
    input: SkillSourceInput,
    endpoints: GithubEndpoints,
) -> Result<SkillSourceResolution, SkillError> {
    let paths = SkillsPaths::from_env().map_err(sanitize_resolution_error)?;
    let operation_id = Uuid::new_v4().hyphenated().to_string();
    let operation_root = paths.staging_skills_dir().join(&operation_id);
    create_private_directory(&operation_root).map_err(sanitize_resolution_error)?;

    let result = match input {
        SkillSourceInput::Github { value } => {
            resolve_github(&value, &endpoints, &operation_id, &operation_root)
        }
        SkillSourceInput::Local { path } => {
            resolve_local(&path, &paths, &operation_id, &operation_root)
        }
    };
    match result {
        Ok(resolution) => Ok(resolution),
        Err(error) => {
            if fs::remove_dir_all(&operation_root).is_err() {
                return Err(SkillError::RecoveryRequired {
                    message:
                        "a failed staged source could not be removed; manual recovery is required"
                            .into(),
                });
            }
            Err(sanitize_resolution_error(error))
        }
    }
}

struct ParsedGithub {
    owner: String,
    repo: String,
    tree: Option<Vec<String>>,
}

struct ResolvedGithub {
    requested_ref: String,
    subpath: String,
    pinned: bool,
    sha: String,
}

fn resolve_github(
    value: &str,
    endpoints: &GithubEndpoints,
    operation_id: &str,
    operation_root: &Path,
) -> Result<SkillSourceResolution, SkillError> {
    let parsed = parse_github_source(value)?;
    validate_endpoint_base(&endpoints.api_base, endpoints)?;
    validate_endpoint_base(&endpoints.archive_base, endpoints)?;
    let agent = github_agent();
    let resolved = resolve_github_metadata(&agent, &parsed, endpoints)?;
    let download_path = operation_root.join("source.tar.gz");
    download_archive(
        &agent,
        &parsed.owner,
        &parsed.repo,
        &resolved.sha,
        endpoints,
        &download_path,
    )?;
    let archive_root = operation_root.join("archive");
    create_private_directory(&archive_root)?;
    let repository_root = extract_archive(&download_path, &archive_root)?;
    let requested_root = join_normalized(&repository_root, &resolved.subpath);
    let candidates = stage_candidates(&requested_root, operation_root)?;
    Ok(SkillSourceResolution {
        operation_id: operation_id.to_owned(),
        source: SkillSource::Github {
            owner: parsed.owner,
            repo: parsed.repo,
            subpath: resolved.subpath,
            requested_ref: resolved.requested_ref,
            pinned: resolved.pinned,
        },
        resolved_revision: Some(resolved.sha),
        candidates,
    })
}

fn resolve_local(
    value: &str,
    paths: &SkillsPaths,
    operation_id: &str,
    operation_root: &Path,
) -> Result<SkillSourceResolution, SkillError> {
    validate_source_text(value)?;
    if value.contains('\0') {
        return invalid_source("local source paths cannot contain NUL bytes");
    }
    let selected = paths
        .expand_user(value)
        .ok_or_else(|| invalid_source_error("the local source path is not valid"))?;
    let selected = lexical_absolute(&selected)?;
    let canonical = fs::canonicalize(&selected).map_err(|error| io_error(&selected, error))?;
    let metadata = fs::metadata(&canonical).map_err(|error| io_error(&canonical, error))?;
    if !metadata.is_dir() {
        return Err(SkillError::InvalidSource {
            message: "the selected local source must be a directory".into(),
        });
    }
    let display_path = collapse_home(&selected, paths.user_home());
    let candidates = stage_candidates(&canonical, operation_root)?;
    Ok(SkillSourceResolution {
        operation_id: operation_id.to_owned(),
        source: SkillSource::Local {
            path: display_path,
            subpath: String::new(),
        },
        resolved_revision: None,
        candidates,
    })
}

fn parse_github_source(value: &str) -> Result<ParsedGithub, SkillError> {
    validate_source_text(value)?;
    if value.contains('\\') {
        return invalid_source("GitHub source paths cannot contain backslashes");
    }
    if !value.contains("://") {
        if value.contains(['@', ':', '?', '#']) {
            return invalid_source("only public owner/repository GitHub sources are supported");
        }
        let raw = value.split('/').collect::<Vec<_>>();
        if raw.len() != 2 {
            return invalid_source("GitHub shorthand must be owner/repository");
        }
        let owner = decode_source_component(raw[0])?;
        let repo = decode_source_component(raw[1])?;
        validate_repository_components(&owner, &repo)?;
        return Ok(ParsedGithub {
            owner,
            repo,
            tree: None,
        });
    }

    let url = Url::parse(value)
        .map_err(|_| invalid_source_error("the GitHub source URL is not valid"))?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return invalid_source("only credential-free public https://github.com URLs are supported");
    }
    let raw_path = source_url_path(value)?;
    let mut raw = raw_path.split('/').skip(1).collect::<Vec<_>>();
    if raw.last() == Some(&"") {
        raw.pop();
    }
    if raw.iter().any(|component| component.is_empty()) || raw.len() < 2 {
        return invalid_source("the GitHub URL must contain an owner and repository");
    }
    let components = raw
        .into_iter()
        .map(decode_source_component)
        .collect::<Result<Vec<_>, _>>()?;
    let owner = components[0].clone();
    let repo = components[1].clone();
    validate_repository_components(&owner, &repo)?;
    let tree = if components.len() == 2 {
        None
    } else {
        if components.get(2).map(String::as_str) != Some("tree") || components.len() == 3 {
            return invalid_source("GitHub URLs may only select a repository or tree path");
        }
        let tree = components[3..].to_vec();
        if tree.len() > MAX_REF_PROBES && !is_sha(&tree[0]) {
            return invalid_source("the tree URL would require too many ref probes");
        }
        Some(tree)
    };
    Ok(ParsedGithub { owner, repo, tree })
}

fn validate_source_text(value: &str) -> Result<(), SkillError> {
    if value.is_empty() || value.len() > MAX_SOURCE_BYTES || value.trim() != value {
        return invalid_source("the source must contain 1 to 4096 bytes without outer whitespace");
    }
    Ok(())
}

fn source_url_path(value: &str) -> Result<&str, SkillError> {
    let authority = value
        .find("://")
        .map(|index| index + 3)
        .ok_or_else(|| invalid_source_error("the GitHub source URL is not valid"))?;
    let path = value[authority..].find('/').map(|index| authority + index);
    Ok(path.map(|index| &value[index..]).unwrap_or("/"))
}

fn decode_source_component(value: &str) -> Result<String, SkillError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(high) = bytes.get(index + 1).and_then(|value| hex_value(*value)) else {
                return invalid_source("source path percent encoding is not canonical");
            };
            let Some(low) = bytes.get(index + 2).and_then(|value| hex_value(*value)) else {
                return invalid_source("source path percent encoding is not canonical");
            };
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(decoded)
        .map_err(|_| invalid_source_error("source path components must be valid UTF-8"))?;
    if decoded.is_empty()
        || matches!(decoded.as_str(), "." | "..")
        || decoded.contains(['/', '\\', '\0'])
        || decoded.chars().any(char::is_control)
    {
        return invalid_source("source path components are not safe");
    }
    Ok(decoded)
}

fn validate_repository_components(owner: &str, repo: &str) -> Result<(), SkillError> {
    if owner.is_empty()
        || repo.is_empty()
        || repo.to_ascii_lowercase().ends_with(".git")
        || [owner, repo].into_iter().any(|component| {
            component
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        })
    {
        return invalid_source("the GitHub owner or repository is not valid");
    }
    Ok(())
}

fn resolve_github_metadata(
    agent: &ureq::Agent,
    parsed: &ParsedGithub,
    endpoints: &GithubEndpoints,
) -> Result<ResolvedGithub, SkillError> {
    if let Some(tree) = &parsed.tree {
        if is_sha(&tree[0]) {
            let sha = resolve_commit(agent, parsed, &tree[0], endpoints)?.ok_or_else(|| {
                invalid_source_error("the requested public GitHub revision was not found")
            })?;
            return Ok(ResolvedGithub {
                requested_ref: tree[0].clone(),
                subpath: tree[1..].join("/"),
                pinned: true,
                sha,
            });
        }
        for length in (1..=tree.len()).rev().take(MAX_REF_PROBES) {
            let requested_ref = tree[..length].join("/");
            if let Some(sha) = resolve_commit(agent, parsed, &requested_ref, endpoints)? {
                return Ok(ResolvedGithub {
                    requested_ref,
                    subpath: tree[length..].join("/"),
                    pinned: false,
                    sha,
                });
            }
        }
        return invalid_source("the requested public GitHub revision was not found");
    }

    let url = endpoint_url(&endpoints.api_base, &["repos", &parsed.owner, &parsed.repo])?;
    let response = fetch_response(agent, url, endpoints)?;
    ensure_public_status(&response, "repository")?;
    #[derive(Deserialize)]
    struct Repository {
        default_branch: String,
    }
    let repository: Repository = read_json(response)?;
    let requested_ref = decode_api_ref(&repository.default_branch)?;
    let sha = resolve_commit(agent, parsed, &requested_ref, endpoints)?.ok_or_else(|| {
        invalid_source_error("the default branch of the public repository was not found")
    })?;
    Ok(ResolvedGithub {
        requested_ref,
        subpath: String::new(),
        pinned: false,
        sha,
    })
}

fn resolve_commit(
    agent: &ureq::Agent,
    parsed: &ParsedGithub,
    requested_ref: &str,
    endpoints: &GithubEndpoints,
) -> Result<Option<String>, SkillError> {
    let url = endpoint_url(
        &endpoints.api_base,
        &[
            "repos",
            &parsed.owner,
            &parsed.repo,
            "commits",
            requested_ref,
        ],
    )?;
    let response = fetch_response(agent, url, endpoints)?;
    if response.status() == 404 {
        return Ok(None);
    }
    ensure_public_status(&response, "revision")?;
    #[derive(Deserialize)]
    struct Commit {
        sha: String,
    }
    let commit: Commit = read_json(response)?;
    if !is_sha(&commit.sha) {
        return invalid_source("GitHub returned a non-canonical commit SHA");
    }
    Ok(Some(commit.sha.to_ascii_lowercase()))
}

fn decode_api_ref(value: &str) -> Result<String, SkillError> {
    if value.is_empty()
        || value.len() > MAX_SOURCE_BYTES
        || value.contains(['\0', '\\'])
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return invalid_source("GitHub returned an invalid default branch");
    }
    Ok(value.to_owned())
}

fn is_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn github_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .build()
}

fn endpoint_url(base: &Url, segments: &[&str]) -> Result<Url, SkillError> {
    let mut url = base.clone();
    url.set_query(None);
    url.set_fragment(None);
    let mut path = url.path_segments_mut().map_err(|_| {
        invalid_source_error("the GitHub endpoint base cannot contain path segments")
    })?;
    path.pop_if_empty();
    path.extend(segments.iter().copied());
    drop(path);
    Ok(url)
}

fn validate_endpoint_base(url: &Url, endpoints: &GithubEndpoints) -> Result<(), SkillError> {
    validate_request_url(url, endpoints)
}

fn validate_request_url(url: &Url, endpoints: &GithubEndpoints) -> Result<(), SkillError> {
    let Some(host) = url.host_str() else {
        return invalid_source("GitHub endpoint URLs must have a host");
    };
    if !endpoints.allowed_hosts.contains(host)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return invalid_source("a GitHub request or redirect left the approved public hosts");
    }
    if url.scheme() == "https" {
        return Ok(());
    }
    if endpoints.allow_http_loopback
        && url.scheme() == "http"
        && matches!(host, "127.0.0.1" | "localhost" | "::1")
    {
        return Ok(());
    }
    invalid_source("GitHub requests and redirects must use HTTPS")
}

fn fetch_response(
    agent: &ureq::Agent,
    mut url: Url,
    endpoints: &GithubEndpoints,
) -> Result<ureq::Response, SkillError> {
    let mut redirects = 0_usize;
    loop {
        validate_request_url(&url, endpoints)?;
        let call = agent
            .get(url.as_str())
            .set("User-Agent", concat!("MUX/", env!("CARGO_PKG_VERSION")))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .call();
        let response = match call {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(_)) => {
                return Err(SkillError::Network {
                    message: "the public GitHub request could not be completed".into(),
                    retry_at: None,
                })
            }
        };
        if matches!(response.status(), 301 | 302 | 303 | 307 | 308) {
            if redirects >= MAX_REDIRECTS {
                return invalid_source("the GitHub redirect limit was exceeded");
            }
            let location = response
                .header("Location")
                .filter(|value| !value.is_empty() && value.len() <= MAX_SOURCE_BYTES)
                .ok_or_else(|| {
                    invalid_source_error("the GitHub redirect is missing a safe location")
                })?;
            url = url
                .join(location)
                .map_err(|_| invalid_source_error("the GitHub redirect location is invalid"))?;
            redirects += 1;
            continue;
        }
        return Ok(response);
    }
}

fn ensure_public_status(response: &ureq::Response, resource: &str) -> Result<(), SkillError> {
    match response.status() {
        200..=299 => Ok(()),
        401 | 403 | 404 => invalid_source(&format!(
            "the {resource} is unavailable as an unauthenticated public GitHub source"
        )),
        429 => Err(SkillError::Network {
            message: "GitHub rate-limited the public source request".into(),
            retry_at: response.header("Retry-After").map(capped_message),
        }),
        _ => Err(SkillError::Network {
            message: "GitHub returned an unsuccessful response for the public source".into(),
            retry_at: None,
        }),
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(response: ureq::Response) -> Result<T, SkillError> {
    let bytes = read_response_bounded(response, MAX_METADATA_BYTES, "metadata")?;
    serde_json::from_slice(&bytes)
        .map_err(|_| invalid_source_error("GitHub returned invalid public source metadata"))
}

fn read_response_bounded(
    response: ureq::Response,
    maximum: u64,
    limit: &'static str,
) -> Result<Vec<u8>, SkillError> {
    if let Some(length) = response.header("Content-Length") {
        let length = length
            .parse::<u64>()
            .map_err(|_| invalid_source_error("the response Content-Length is invalid"))?;
        enforce_limit(limit, length, maximum)?;
    }
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let remaining = maximum.saturating_add(1).saturating_sub(total);
        let requested = buffer.len().min(remaining as usize);
        if requested == 0 {
            return Err(limit_error(limit, total.saturating_add(1), maximum));
        }
        let read = reader
            .read(&mut buffer[..requested])
            .map_err(|_| SkillError::Network {
                message: "the public GitHub response ended unexpectedly".into(),
                retry_at: None,
            })?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        enforce_limit(limit, total, maximum)?;
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

fn download_archive(
    agent: &ureq::Agent,
    owner: &str,
    repo: &str,
    sha: &str,
    endpoints: &GithubEndpoints,
    destination: &Path,
) -> Result<(), SkillError> {
    let url = endpoint_url(&endpoints.archive_base, &[owner, repo, "tar.gz", sha])?;
    let response = fetch_response(agent, url, endpoints)?;
    ensure_public_status(&response, "archive")?;
    let declared = response
        .header("Content-Length")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| invalid_source_error("the archive Content-Length is invalid"))
        })
        .transpose()?;
    if let Some(declared) = declared {
        enforce_limit("download", declared, MAX_DOWNLOAD_BYTES)?;
    }
    let mut source = response.into_reader();
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| io_error(destination, error))?;
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let remaining = MAX_DOWNLOAD_BYTES.saturating_add(1).saturating_sub(total);
        let requested = buffer.len().min(remaining as usize);
        if requested == 0 {
            return Err(limit_error(
                "download",
                total.saturating_add(1),
                MAX_DOWNLOAD_BYTES,
            ));
        }
        let read = source
            .read(&mut buffer[..requested])
            .map_err(|_| SkillError::Network {
                message: "the public GitHub archive download ended unexpectedly".into(),
                retry_at: None,
            })?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        enforce_limit("download", total, MAX_DOWNLOAD_BYTES)?;
        output
            .write_all(&buffer[..read])
            .map_err(|error| io_error(destination, error))?;
    }
    if declared.is_some_and(|declared| declared != total) {
        return invalid_source("the archive size did not match its Content-Length");
    }
    output.flush().map_err(|error| io_error(destination, error))
}

struct ExpansionReader<R> {
    inner: R,
    total: Arc<AtomicU64>,
}

impl<R: Read> Read for ExpansionReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let current = self.total.load(Ordering::Relaxed);
        let remaining = MAX_ARCHIVE_BYTES.saturating_add(1).saturating_sub(current);
        if remaining == 0 {
            self.total
                .store(MAX_ARCHIVE_BYTES.saturating_add(1), Ordering::Relaxed);
            return Err(std::io::Error::other("archive expansion limit exceeded"));
        }
        let requested = buffer.len().min(remaining as usize);
        let read = self.inner.read(&mut buffer[..requested])?;
        let total = current.saturating_add(read as u64);
        self.total.store(total, Ordering::Relaxed);
        if total > MAX_ARCHIVE_BYTES {
            return Err(std::io::Error::other("archive expansion limit exceeded"));
        }
        Ok(read)
    }
}

struct PendingSymlink {
    relative: String,
    target: String,
    destination: PathBuf,
}

fn extract_archive(download: &Path, destination: &Path) -> Result<PathBuf, SkillError> {
    let file = File::open(download).map_err(|error| io_error(download, error))?;
    let total = Arc::new(AtomicU64::new(0));
    let decoder = GzDecoder::new(file);
    let reader = ExpansionReader {
        inner: decoder,
        total: Arc::clone(&total),
    };
    let mut archive = Archive::new(reader);
    let entries = archive.entries().map_err(|_| archive_read_error(&total))?;
    let mut seen = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut pending_symlinks = Vec::new();
    let mut entry_count = 0_u64;
    let mut content_bytes = 0_u64;

    for entry in entries {
        entry_count = entry_count.saturating_add(1);
        enforce_limit("entries", entry_count, MAX_ARCHIVE_ENTRIES)?;
        let mut entry = entry.map_err(|_| archive_read_error(&total))?;
        let kind = entry.header().entry_type();
        let components = normalize_archive_path(entry.path_bytes().as_ref(), kind.is_dir())?;
        let relative = components.join("/");
        if !seen.insert(relative.clone()) {
            return invalid_source("the archive contains duplicate entry paths");
        }
        roots.insert(components[0].clone());
        if roots.len() > 1 {
            return invalid_source("the archive contains multiple repository roots");
        }
        let destination_path = components
            .iter()
            .fold(destination.to_path_buf(), |path, component| {
                path.join(component)
            });
        ensure_archive_parents(destination, &components[..components.len() - 1])?;
        let declared = entry.size();
        if kind.is_file() {
            enforce_limit("single_file", declared, MAX_SINGLE_FILE_BYTES)?;
            content_bytes = content_bytes.saturating_add(declared);
            enforce_limit("archive", content_bytes, MAX_ARCHIVE_BYTES)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination_path)
                .map_err(|_| invalid_source_error("archive entries collide after normalization"))?;
            let actual = copy_archive_entry(&mut entry, &mut output, declared, &total)?;
            if actual != declared {
                return invalid_source("an archive entry size did not match its header");
            }
            output
                .flush()
                .map_err(|error| io_error(&destination_path, error))?;
            set_archive_file_mode(&destination_path, entry.header().mode().unwrap_or(0o644))?;
        } else if kind.is_dir() {
            if declared != 0 {
                return invalid_source("archive directory entries must have zero size");
            }
            ensure_archive_directory(&destination_path)?;
        } else if kind.is_symlink() {
            if declared != 0 {
                return invalid_source("archive symlink entries must have zero size");
            }
            let target = entry
                .link_name_bytes()
                .ok_or_else(|| invalid_source_error("an archive symlink is missing its target"))?;
            if target.is_empty() || target.len() > MAX_SOURCE_BYTES || target.contains(&0) {
                return invalid_source("an archive symlink target is not valid");
            }
            let target = std::str::from_utf8(target.as_ref())
                .map_err(|_| invalid_source_error("archive symlink targets must be valid UTF-8"))?
                .to_owned();
            pending_symlinks.push(PendingSymlink {
                relative,
                target,
                destination: destination_path,
            });
        } else if kind.is_hard_link() {
            return invalid_source("hard links are not allowed in Skill archives");
        } else {
            return invalid_source("special files are not allowed in Skill archives");
        }
    }
    if total.load(Ordering::Relaxed) > MAX_ARCHIVE_BYTES {
        return Err(limit_error(
            "archive",
            total.load(Ordering::Relaxed),
            MAX_ARCHIVE_BYTES,
        ));
    }
    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| invalid_source_error("the public GitHub archive is empty"))?;
    for link in &pending_symlinks {
        create_archive_symlink(&link.target, &link.destination)?;
    }
    validate_archive_symlinks(destination, &pending_symlinks)?;
    Ok(destination.join(root))
}

fn normalize_archive_path(bytes: &[u8], directory: bool) -> Result<Vec<String>, SkillError> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_BYTES || bytes.contains(&0) {
        return invalid_source("an archive entry path is not valid");
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid_source_error("archive entry paths must be valid UTF-8"))?;
    if text.starts_with(['/', '\\']) {
        return invalid_source("absolute archive entry paths are not allowed");
    }
    let text = if directory {
        text.strip_suffix('/').unwrap_or(text)
    } else {
        text
    };
    let components = text
        .split('/')
        .map(|component| component.to_owned())
        .collect::<Vec<_>>();
    if components.is_empty()
        || components.iter().any(|component| {
            component.is_empty()
                || matches!(component.as_str(), "." | "..")
                || component.contains(['\\', '\0'])
        })
        || !Path::new(text)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return invalid_source("archive entry paths must use safe canonical components");
    }
    Ok(components)
}

fn ensure_archive_parents(root: &Path, components: &[String]) -> Result<(), SkillError> {
    let mut current = root.to_path_buf();
    for component in components {
        current.push(component);
        ensure_archive_directory(&current)?;
    }
    Ok(())
}

fn ensure_archive_directory(path: &Path) -> Result<(), SkillError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => return Ok(()),
        Ok(_) => return invalid_source("archive entries collide after normalization"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(path, error)),
    }
    fs::create_dir(path).map_err(|error| io_error(path, error))?;
    set_private_mode(path)
}

fn copy_archive_entry(
    entry: &mut impl Read,
    output: &mut File,
    declared: u64,
    expansion: &Arc<AtomicU64>,
) -> Result<u64, SkillError> {
    let mut actual = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let remaining = MAX_SINGLE_FILE_BYTES
            .saturating_add(1)
            .saturating_sub(actual);
        let requested = buffer.len().min(remaining as usize);
        if requested == 0 {
            return Err(limit_error(
                "single_file",
                actual.saturating_add(1),
                MAX_SINGLE_FILE_BYTES,
            ));
        }
        let read = entry
            .read(&mut buffer[..requested])
            .map_err(|_| archive_read_error(expansion))?;
        if read == 0 {
            break;
        }
        actual = actual.saturating_add(read as u64);
        enforce_limit("single_file", actual, MAX_SINGLE_FILE_BYTES)?;
        output
            .write_all(&buffer[..read])
            .map_err(|_| invalid_source_error("an archive entry could not be staged"))?;
    }
    if actual != declared {
        return invalid_source("an archive entry size did not match its header");
    }
    Ok(actual)
}

fn archive_read_error(total: &Arc<AtomicU64>) -> SkillError {
    let actual = total.load(Ordering::Relaxed);
    if actual > MAX_ARCHIVE_BYTES {
        limit_error("archive", actual, MAX_ARCHIVE_BYTES)
    } else {
        invalid_source_error("the public GitHub archive is malformed or truncated")
    }
}

#[cfg(unix)]
fn create_archive_symlink(target: &str, destination: &Path) -> Result<(), SkillError> {
    std::os::unix::fs::symlink(target, destination)
        .map_err(|_| invalid_source_error("archive symlink entries collide after normalization"))
}

#[cfg(windows)]
fn create_archive_symlink(target: &str, destination: &Path) -> Result<(), SkillError> {
    let resolved = destination
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(target);
    let result = if resolved.is_dir() {
        std::os::windows::fs::symlink_dir(target, destination)
    } else {
        std::os::windows::fs::symlink_file(target, destination)
    };
    result.map_err(|_| invalid_source_error("archive symlink entries collide after normalization"))
}

#[cfg(all(not(unix), not(windows)))]
fn create_archive_symlink(_target: &str, _destination: &Path) -> Result<(), SkillError> {
    invalid_source("secure archive symlink extraction is unavailable on this platform")
}

#[cfg(unix)]
fn validate_archive_symlinks(root: &Path, links: &[PendingSymlink]) -> Result<(), SkillError> {
    let anchored = AnchoredRoot::open(root)?;
    for link in links {
        anchored
            .validate_symlink_target(&link.relative, &link.target, &link.destination)
            .map_err(|error| match error {
                error @ SkillError::LimitExceeded { .. } => error,
                _ => invalid_source_error(
                    "an archive symlink does not resolve safely inside the extracted tree",
                ),
            })?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_archive_symlinks(_root: &Path, links: &[PendingSymlink]) -> Result<(), SkillError> {
    if links.is_empty() {
        Ok(())
    } else {
        invalid_source("secure archive symlink validation is unavailable on this platform")
    }
}

fn stage_candidates(
    source_root: &Path,
    operation_root: &Path,
) -> Result<Vec<SkillCandidateSummary>, SkillError> {
    let discovered = discover_candidates(source_root)?;
    if discovered.is_empty() {
        return invalid_source("the selected source contains no valid Skill candidates");
    }
    reject_nested_candidates(&discovered)?;
    let mut prepared = Vec::with_capacity(discovered.len());
    let mut names = BTreeMap::new();
    let mut aggregate = 0_u64;
    for relative in discovered {
        let path = join_normalized(source_root, &relative);
        let validated = validate_candidate(&path)?;
        if let Some(previous) = names.insert(validated.manifest.name.clone(), relative.clone()) {
            return Err(SkillError::Conflict {
                message: format!(
                    "duplicate Skill name '{}' was found at '{}' and '{}'",
                    validated.manifest.name, previous, relative
                ),
                path: String::new(),
            });
        }
        aggregate = aggregate.saturating_add(validated.total_bytes);
        enforce_limit("archive", aggregate, MAX_ARCHIVE_BYTES)?;
        prepared.push((relative, path, validated));
    }
    prepared.sort_by(|left, right| {
        left.2
            .manifest
            .name
            .cmp(&right.2.manifest.name)
            .then_with(|| left.0.cmp(&right.0))
    });

    let candidates_root = operation_root.join("candidates");
    create_private_directory(&candidates_root)?;
    let mut summaries = Vec::with_capacity(prepared.len());
    for (relative, source, before) in prepared {
        let destination = candidates_root.join(&before.manifest.name);
        copy_tree_secure(&source, &destination)?;
        let staged = validate_candidate(&destination)?;
        if staged.manifest.name != before.manifest.name
            || staged.content_hash != before.content_hash
            || staged.total_bytes != before.total_bytes
        {
            return Err(SkillError::Conflict {
                message: "a Skill candidate changed while its snapshot was staged".into(),
                path: String::new(),
            });
        }
        summaries.push(SkillCandidateSummary {
            name: staged.manifest.name,
            description: staged.manifest.description,
            relative_path: relative,
            content_kind: staged.content_kind,
            content_hash: staged.content_hash,
            file_count: staged.files.len() as u64,
            total_bytes: staged.total_bytes,
        });
    }
    Ok(summaries)
}

fn discover_candidates(root: &Path) -> Result<Vec<String>, SkillError> {
    let anchored = AnchoredRoot::open(root)?;
    let root_directory = anchored.root_directory()?;
    if directory_has_manifest(&anchored, &root_directory, anchored.canonical_path())? {
        return Ok(vec![String::new()]);
    }
    let mut count = 0_u64;
    let mut pending = vec![(root_directory, String::new())];
    let mut candidates = Vec::new();
    while let Some((directory, relative)) = pending.pop() {
        let directory_path = join_normalized(anchored.canonical_path(), &relative);
        let names = anchored.read_directory_budgeted(
            &directory,
            &directory_path,
            count,
            MAX_ARCHIVE_ENTRIES,
            "entries",
        )?;
        count = count.saturating_add(names.len() as u64);
        let mut has_manifest = false;
        let mut children = Vec::new();
        for name in names {
            let name_text = std::str::from_utf8(name.to_bytes()).map_err(|_| {
                invalid_source_error("Skill source entry paths must be valid UTF-8")
            })?;
            let child_relative = if relative.is_empty() {
                name_text.to_owned()
            } else {
                format!("{relative}/{name_text}")
            };
            let child_path = join_normalized(anchored.canonical_path(), &child_relative);
            let identity = anchored.stat_entry(&directory, &name, &child_path)?;
            if name_text == "SKILL.md" {
                has_manifest = true;
            }
            if identity.kind == AnchoredFileKind::Directory {
                let child =
                    anchored.open_directory_entry(&directory, &name, &identity, &child_path)?;
                children.push((child, child_relative));
            }
        }
        if has_manifest {
            candidates.push(relative.clone());
        }
        children.reverse();
        pending.extend(children);
    }
    candidates.sort();
    Ok(candidates)
}

fn directory_has_manifest(
    root: &AnchoredRoot,
    directory: &File,
    path: &Path,
) -> Result<bool, SkillError> {
    let names = root.read_directory_budgeted(directory, path, 0, MAX_ARCHIVE_ENTRIES, "entries")?;
    Ok(names.iter().any(|name| name.to_bytes() == b"SKILL.md"))
}

fn reject_nested_candidates(candidates: &[String]) -> Result<(), SkillError> {
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.is_empty() {
            continue;
        }
        let prefix = format!("{candidate}/");
        if candidates
            .iter()
            .skip(index + 1)
            .any(|other| other.starts_with(&prefix))
        {
            return Err(SkillError::Conflict {
                message: "nested Skill candidates would create overlapping snapshots".into(),
                path: String::new(),
            });
        }
    }
    Ok(())
}

fn join_normalized(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|component| !component.is_empty())
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

fn collapse_home(path: &Path, home: &Path) -> String {
    if path == home {
        return "~".into();
    }
    if let Ok(relative) = path.strip_prefix(home) {
        return format!("~/{}", normalized_path(relative));
    }
    normalized_path(path)
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn lexical_absolute(path: &Path) -> Result<PathBuf, SkillError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| io_error(path, error))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str())
            }
        }
    }
    Ok(normalized)
}

fn create_private_directory(path: &Path) -> Result<(), SkillError> {
    fs::create_dir(path).map_err(|error| io_error(path, error))?;
    set_private_mode(path)
}

fn set_private_mode(path: &Path) -> Result<(), SkillError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error(path, error))?;
    }
    Ok(())
}

fn set_archive_file_mode(path: &Path, archive_mode: u32) -> Result<(), SkillError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = 0o600 | (archive_mode & 0o111);
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|error| io_error(path, error))?;
    }
    Ok(())
}

fn enforce_limit(limit: &'static str, actual: u64, allowed: u64) -> Result<(), SkillError> {
    if actual > allowed {
        return Err(limit_error(limit, actual, allowed));
    }
    Ok(())
}

fn limit_error(limit: &'static str, actual: u64, allowed: u64) -> SkillError {
    SkillError::LimitExceeded {
        limit: limit.into(),
        actual,
        allowed,
    }
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn invalid_source<T>(message: &str) -> Result<T, SkillError> {
    Err(invalid_source_error(message))
}

fn invalid_source_error(message: &str) -> SkillError {
    SkillError::InvalidSource {
        message: capped_message(message),
    }
}

fn sanitize_resolution_error(error: SkillError) -> SkillError {
    match error {
        SkillError::InvalidManifest { message, .. } => SkillError::InvalidManifest {
            message: capped_message(message),
            path: String::new(),
        },
        SkillError::UnsafePath { message, .. } => SkillError::UnsafePath {
            message: capped_message(message),
            path: String::new(),
        },
        SkillError::Conflict { message, .. } => SkillError::Conflict {
            message: capped_message(message),
            path: String::new(),
        },
        SkillError::Io { message, .. } => SkillError::Io {
            message: capped_message(message),
            path: None,
        },
        SkillError::InvalidSource { message } => SkillError::InvalidSource {
            message: capped_message(message),
        },
        SkillError::Network { message, retry_at } => SkillError::Network {
            message: capped_message(message),
            retry_at: retry_at.map(capped_message),
        },
        SkillError::PlanStale { message } => SkillError::PlanStale {
            message: capped_message(message),
        },
        SkillError::ConfirmationRequired {
            message,
            findings_hash,
        } => SkillError::ConfirmationRequired {
            message: capped_message(message),
            findings_hash,
        },
        SkillError::RecoveryRequired { message } => SkillError::RecoveryRequired {
            message: capped_message(message),
        },
        error @ SkillError::LimitExceeded { .. } => error,
    }
}
