#![allow(dead_code)]

use flate2::write::GzEncoder;
use flate2::Compression;
use mux_core::settings::{load_settings, mutate_settings};
use mux_core::skills::{
    crash_transaction_at_phase_for_test, hash_tree, DirectoryMutation, GithubEndpoints,
    InventoryState, JournalPhase, LinkMutation, LinkState, ManagedSkillRecord, RiskLevel,
    SkillContentKind, SkillRiskSummary, SkillSettingsSnapshot, SkillSource, SkillUpdateState,
    SkillsInventory, SkillsPaths, TransactionOrder, TransactionSpec,
};
use mux_core::testenv::TestHome;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tar::{Builder, Header};

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};
#[cfg(windows)]
use std::os::windows::fs::symlink_dir as symlink;

pub const FIXTURE_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
pub const TRANSACTION_OPERATION_ID: &str = "00000000-0000-4000-8000-000000000006";

pub type FixtureSnapshot = BTreeMap<String, Vec<u8>>;

pub struct TransactionFixture {
    pub home: TestHome,
    pub paths: SkillsPaths,
    pub before_snapshot: FixtureSnapshot,
    pub spec: TransactionSpec,
}

impl TransactionFixture {
    pub fn managed(name: &str) -> Self {
        let home = TestHome::new(&format!("tx-{name}"));
        fs::create_dir_all(home.home.join(".codex")).unwrap();
        let paths = SkillsPaths::from_env().unwrap();
        let central = paths.central_skill(name);
        write_skill(&central, name, "Managed fixture");
        let before_hash = hash_tree(&central).unwrap();

        let target = home.home.join(".agents/skills").join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        symlink(&central, &target).unwrap();

        let before_record = managed_record(name, &before_hash);
        let settings_before = SkillSettingsSnapshot {
            managed_skills: Some(BTreeMap::from([(name.to_owned(), before_record.clone())])),
            skill_assignments: Some(BTreeMap::from([(
                name.to_owned(),
                BTreeSet::from(["agents-user".to_owned()]),
            )])),
            skill_update_checked_at: Some("2026-07-16T00:00:00Z".into()),
        };
        mutate_settings(|settings| {
            settings.managed_skills = settings_before.managed_skills.clone();
            settings.skill_assignments = settings_before.skill_assignments.clone();
            settings.skill_update_checked_at = settings_before.skill_update_checked_at.clone();
        })
        .unwrap();

        let replacement = paths
            .staging_skills_dir()
            .join(TRANSACTION_OPERATION_ID)
            .join("candidates")
            .join(name);
        write_skill(&replacement, name, "Updated fixture");
        fs::create_dir_all(replacement.join("scripts")).unwrap();
        fs::write(replacement.join("scripts/run.sh"), b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            fs::set_permissions(&replacement, fs::Permissions::from_mode(0o700)).unwrap();
            fs::set_permissions(
                replacement.join("scripts"),
                fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            fs::set_permissions(
                replacement.join("SKILL.md"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
            fs::set_permissions(
                replacement.join("scripts/run.sh"),
                fs::Permissions::from_mode(0o711),
            )
            .unwrap();
        }
        let replacement_hash = hash_tree(&replacement).unwrap();

        let mut after_record = before_record;
        after_record.description = "Updated fixture".into();
        after_record.content_hash = replacement_hash;
        after_record.updated_at = "2026-07-17T00:00:00Z".into();
        let settings_after = SkillSettingsSnapshot {
            managed_skills: Some(BTreeMap::from([(name.to_owned(), after_record)])),
            skill_assignments: settings_before.skill_assignments.clone(),
            skill_update_checked_at: Some("2026-07-17T00:00:00Z".into()),
        };
        let backup = paths
            .backups_skills_dir()
            .join(TRANSACTION_OPERATION_ID)
            .join(name);
        let spec = TransactionSpec {
            operation_id: TRANSACTION_OPERATION_ID.into(),
            order: TransactionOrder::ContentThenLinks,
            directory_mutations: vec![DirectoryMutation {
                replacement: Some(replacement),
                destination: central.clone(),
                backup,
                expected_before_hash: Some(before_hash),
            }],
            link_mutations: vec![LinkMutation {
                path: target,
                expected: LinkState::ManagedSymlink {
                    target: central.clone(),
                },
                desired_target: Some(central),
                backup: None,
            }],
            settings_before,
            settings_after,
        };
        let mut fixture = Self {
            home,
            paths,
            before_snapshot: BTreeMap::new(),
            spec,
        };
        fixture.before_snapshot = fixture.snapshot();
        fixture
    }

    pub fn crashed_at(phase: JournalPhase) -> Self {
        let fixture = Self::managed("recovery");
        crash_transaction_at_phase_for_test(fixture.spec.clone(), phase).unwrap();
        fixture
    }

    pub fn snapshot(&self) -> FixtureSnapshot {
        let mut snapshot = BTreeMap::new();
        for (index, mutation) in self.spec.directory_mutations.iter().enumerate() {
            capture_entry(
                &mut snapshot,
                &format!("destination:{index}"),
                &mutation.destination,
            );
            capture_entry(&mut snapshot, &format!("backup:{index}"), &mutation.backup);
        }
        for (index, mutation) in self.spec.link_mutations.iter().enumerate() {
            capture_entry(&mut snapshot, &format!("link:{index}"), &mutation.path);
        }
        let settings = load_settings();
        snapshot.insert(
            "settings:skills".into(),
            serde_json::to_vec(&(
                settings.managed_skills,
                settings.skill_assignments,
                settings.skill_update_checked_at,
            ))
            .unwrap(),
        );
        snapshot
    }

    pub fn update_spec(&self) -> TransactionSpec {
        self.spec.clone()
    }
}

fn capture_entry(snapshot: &mut FixtureSnapshot, label: &str, path: &Path) {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            snapshot.insert(label.into(), b"missing".to_vec());
            return;
        }
        Err(error) => panic!("snapshot {label}: {error}"),
    };
    if metadata.file_type().is_symlink() {
        let mut value = b"symlink:".to_vec();
        value.extend_from_slice(fs::read_link(path).unwrap().as_os_str().as_encoded_bytes());
        snapshot.insert(label.into(), value);
        return;
    }
    if metadata.is_file() {
        let mut value = format!("file:{:o}:", permission_mode(&metadata)).into_bytes();
        value.extend_from_slice(&fs::read(path).unwrap());
        snapshot.insert(label.into(), value);
        return;
    }
    assert!(
        metadata.is_dir(),
        "fixture snapshots accept only files and directories"
    );
    snapshot.insert(
        label.into(),
        format!("directory:{:o}", permission_mode(&metadata)).into_bytes(),
    );
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        capture_entry(
            snapshot,
            &format!("{label}/{}", entry.file_name().to_string_lossy()),
            &entry.path(),
        );
    }
}

#[cfg(unix)]
fn permission_mode(metadata: &fs::Metadata) -> u32 {
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn permission_mode(_metadata: &fs::Metadata) -> u32 {
    0
}

enum MockMode {
    Skills(Vec<String>),
    Redirect(String),
    Oversized(u64),
}

struct MockGithubState {
    mode: MockMode,
    archive: Vec<u8>,
    requests: Mutex<Vec<String>>,
    stop: AtomicBool,
}

pub struct MockGithub {
    base: String,
    state: Arc<MockGithubState>,
    thread: Option<JoinHandle<()>>,
}

impl MockGithub {
    pub fn start(skill_names: &[&str]) -> Self {
        Self::spawn(MockMode::Skills(
            skill_names.iter().map(|name| (*name).to_owned()).collect(),
        ))
    }

    pub fn redirect_to(url: &str) -> Self {
        Self::spawn(MockMode::Redirect(url.to_owned()))
    }

    pub fn oversized_download(byte_count: u64) -> Self {
        Self::spawn(MockMode::Oversized(byte_count))
    }

    pub fn endpoints(&self) -> GithubEndpoints {
        let base: url::Url = self.base.parse().expect("mock GitHub base URL");
        GithubEndpoints::for_test(base.clone(), base)
    }

    pub fn requests(&self) -> Vec<String> {
        self.state.requests.lock().unwrap().clone()
    }

    fn spawn(mode: MockMode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock GitHub");
        listener
            .set_nonblocking(true)
            .expect("configure mock GitHub listener");
        let address = listener.local_addr().expect("mock GitHub address");
        let skill_names = match &mode {
            MockMode::Skills(names) => names.clone(),
            MockMode::Redirect(_) | MockMode::Oversized(_) => vec!["review".into()],
        };
        let state = Arc::new(MockGithubState {
            mode,
            archive: github_archive(&skill_names),
            requests: Mutex::new(Vec::new()),
            stop: AtomicBool::new(false),
        });
        let thread_state = Arc::clone(&state);
        let thread = thread::spawn(move || {
            while !thread_state.stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => handle_connection(stream, &thread_state),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base: format!("http://{address}/"),
            state,
            thread: Some(thread),
        }
    }
}

impl Drop for MockGithub {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::Release);
        if let Ok(url) = self.base.parse::<url::Url>() {
            if let Some(address) = url
                .socket_addrs(|| None)
                .ok()
                .and_then(|mut rows| rows.pop())
            {
                let _ = TcpStream::connect_timeout(&address, Duration::from_millis(20));
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn github_archive(skill_names: &[String]) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut archive = Builder::new(encoder);
    for name in skill_names {
        let body =
            format!("---\nname: {name}\ndescription: {name} fixture\n---\n\nFixture body.\n");
        let mut header = Header::new_ustar();
        header.set_mode(0o644);
        header.set_size(body.len() as u64);
        header.set_cksum();
        archive
            .append_data(
                &mut header,
                format!("skills-{FIXTURE_SHA}/catalog/{name}/SKILL.md"),
                Cursor::new(body.into_bytes()),
            )
            .expect("append mock Skill");
    }
    let encoder = archive.into_inner().expect("finish mock tar");
    encoder.finish().expect("finish mock gzip")
}

fn handle_connection(mut stream: TcpStream, state: &MockGithubState) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") && request.len() < 64 * 1024 {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&buffer[..read]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let mut lines = request.split("\r\n");
    let target = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect::<BTreeMap<_, _>>();
    if !headers
        .get("user-agent")
        .is_some_and(|value| value.starts_with("MUX/"))
        || headers.get("accept").map(String::as_str) != Some("application/vnd.github+json")
        || headers.get("x-github-api-version").map(String::as_str) != Some("2022-11-28")
        || headers.contains_key("authorization")
        || headers.contains_key("cookie")
    {
        write_response(&mut stream, "400 Bad Request", &[], b"missing safe headers");
        return;
    }

    let path = target.split('?').next().unwrap_or(target);
    if path == "/repos/acme/skills" {
        state.requests.lock().unwrap().push("repo".into());
        write_response(
            &mut stream,
            "200 OK",
            &[("Content-Type", "application/json")],
            br#"{"default_branch":"main"}"#,
        );
        return;
    }
    if let Some(encoded_ref) = path.strip_prefix("/repos/acme/skills/commits/") {
        let requested_ref = decode_percent(encoded_ref).unwrap_or_else(|| encoded_ref.to_owned());
        state
            .requests
            .lock()
            .unwrap()
            .push(format!("commit:{requested_ref}"));
        if requested_ref == "main" || requested_ref == FIXTURE_SHA {
            write_response(
                &mut stream,
                "200 OK",
                &[("Content-Type", "application/json")],
                format!(r#"{{"sha":"{FIXTURE_SHA}"}}"#).as_bytes(),
            );
        } else {
            write_response(&mut stream, "404 Not Found", &[], b"not found");
        }
        return;
    }

    let is_archive = path == format!("/acme/skills/tar.gz/{FIXTURE_SHA}")
        || matches!(&state.mode, MockMode::Redirect(url) if url.starts_with('/') && path == url);
    if is_archive {
        state.requests.lock().unwrap().push("archive".into());
        match &state.mode {
            MockMode::Skills(_) => write_response(
                &mut stream,
                "200 OK",
                &[("Content-Type", "application/gzip")],
                &state.archive,
            ),
            MockMode::Redirect(url) if path != url => {
                write_response(&mut stream, "302 Found", &[("Location", url)], b"")
            }
            MockMode::Redirect(_) => write_response(
                &mut stream,
                "200 OK",
                &[("Content-Type", "application/gzip")],
                &state.archive,
            ),
            MockMode::Oversized(byte_count) => {
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Length: {byte_count}\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(header.as_bytes());
            }
        }
        return;
    }
    write_response(&mut stream, "404 Not Found", &[], b"not found");
}

fn write_response(stream: &mut TcpStream, status: &str, headers: &[(&str, &str)], body: &[u8]) {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn decode_percent(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

pub fn write_skill(root: &Path, name: &str, description: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\nFixture body.\n"),
    )
    .unwrap();
}

pub fn managed_record(name: &str, content_hash: &str) -> ManagedSkillRecord {
    ManagedSkillRecord {
        name: name.into(),
        description: "Managed fixture".into(),
        content_kind: SkillContentKind::Instructions,
        source: SkillSource::Local {
            path: "~/fixtures".into(),
            subpath: name.into(),
        },
        resolved_revision: None,
        content_hash: content_hash.into(),
        installed_at: "2026-07-16T00:00:00Z".into(),
        updated_at: "2026-07-16T00:00:00Z".into(),
        risk: SkillRiskSummary {
            level: RiskLevel::Low,
            findings: Vec::new(),
            finding_count: 0,
            findings_truncated: false,
        },
        update: SkillUpdateState::default(),
    }
}

#[allow(dead_code)]
pub fn assert_managed_link(path: PathBuf, central: PathBuf) {
    assert!(fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::canonicalize(path).unwrap(),
        fs::canonicalize(central).unwrap()
    );
}

pub fn has_state(inventory: &SkillsInventory, name: &str, state: InventoryState) -> bool {
    inventory
        .items
        .iter()
        .any(|item| item.name == name && item.states.contains(&state))
}
