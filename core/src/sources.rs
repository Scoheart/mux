//! Catalog **sources**: subscribed remote URLs and added local files.
//!
//! A source's servers are parsed from a cached copy on disk under
//! `~/.mux/sources/<kind>/<id>.<ext>`. This module owns the parse / fetch /
//! store primitives; the Tauri commands in `commands.rs` orchestrate them.
//! There is no "builtin" source — the catalog is entirely user-driven.

use crate::adapter::get_adapter;
use crate::paths::{local_sources_dir, remote_sources_dir};
use crate::types::{McpConfig, RegistryEntry, RegistryOrigin, SourceDef};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The cached-file path backing a source (`remote/<id>.<ext>` or
/// `local/<id>.<ext>`). `None` for any unrecognized kind.
pub fn cached_path(def: &SourceDef) -> Option<PathBuf> {
    let ext = if def.format == "toml" { "toml" } else { "json" };
    let file = format!("{}.{}", def.id, ext);
    match def.kind.as_str() {
        "remote" => Some(remote_sources_dir().join(file)),
        "local" => Some(local_sources_dir().join(file)),
        _ => None,
    }
}

fn origin_for(def: &SourceDef) -> RegistryOrigin {
    RegistryOrigin {
        kind: def.kind.clone(),
        agent: None,
        scope: None,
        source: Some(def.id.clone()),
    }
}

fn entry_from(name: String, cfg: McpConfig, origin: &RegistryOrigin) -> RegistryEntry {
    let config = cfg.into();
    RegistryEntry {
        name,
        description: String::new(),
        tags: Vec::new(),
        config,
        origin: Some(origin.clone()),
        repo: None,
    }
}

/// Parse a config file into registry entries. Tries the rich MUX array format
/// first (a JSON `[RegistryEntry, …]`, so curated collections keep their
/// descriptions/tags), else the standard `mcpServers` map via the adapter
/// (transport auto-detected by `McpConfig`).
pub fn parse_file(path: &Path, format: &str, key: &str, origin: &RegistryOrigin) -> Vec<RegistryEntry> {
    if format != "toml" {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(arr) = serde_json::from_str::<Vec<RegistryEntry>>(&content) {
                // Preserve an entry's own origin if it carries one (managed
                // manual/discovered files store explicit origins); otherwise tag
                // it with this source's origin.
                return arr
                    .into_iter()
                    .map(|mut e| {
                        if e.origin.is_none() {
                            e.origin = Some(origin.clone());
                        }
                        e
                    })
                    .collect();
            }
        }
    }
    get_adapter(format, key)
        .read(path)
        .into_iter()
        .map(|(name, cfg)| entry_from(name, cfg, origin))
        .collect()
}

/// Entries a source contributes to the catalog (from its cached file).
pub fn source_entries(def: &SourceDef) -> Vec<RegistryEntry> {
    let Some(path) = cached_path(def) else { return Vec::new() };
    if !path.exists() {
        return Vec::new();
    }
    parse_file(&path, &def.format, &def.key, &origin_for(def))
}

/// How many servers a source currently provides.
pub fn source_count(def: &SourceDef) -> u32 {
    source_entries(def).len() as u32
}

/// Fetch a remote URL's body as text (20s connect/read timeout; ureq caps the
/// body size internally).
pub fn fetch(url: &str) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .build();
    agent
        .get(url)
        .call()
        .map_err(|e| format!("抓取失败: {}", e))?
        .into_string()
        .map_err(|e| format!("读取响应失败: {}", e))
}

/// Guess the format from a URL/path hint, else sniff the content (valid JSON ⇒
/// json, otherwise toml).
pub fn detect_format(hint: &str, content: &str) -> &'static str {
    let h = hint.to_lowercase();
    if h.ends_with(".toml") {
        return "toml";
    }
    if h.ends_with(".json") {
        return "json";
    }
    if serde_json::from_str::<serde_json::Value>(content).is_ok() {
        "json"
    } else {
        "toml"
    }
}

/// Reject content that isn't well-formed JSON/TOML before we store it.
pub fn validate_parseable(content: &str, format: &str) -> Result<(), String> {
    let ok = if format == "toml" {
        content.parse::<toml::Value>().is_ok()
    } else {
        serde_json::from_str::<serde_json::Value>(content).is_ok()
    };
    if ok {
        Ok(())
    } else {
        Err("内容不是有效的 JSON / TOML 配置".into())
    }
}

/// Write a source's cached file (creating parent dirs). Plain write is fine — the
/// file is a disposable cache, re-derivable by refresh.
pub fn write_source_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, content).map_err(|e| e.to_string())
}

/// Current local time as an ISO-ish stamp.
pub fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// A collision-resistant source id: `<kind>-<slug>-<timestamp-ms>`.
pub fn gen_id(kind: &str, label: &str) -> String {
    let slug: String = label
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(12)
        .collect::<String>()
        .to_lowercase();
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S%3f");
    if slug.is_empty() {
        format!("{}-{}", kind, ts)
    } else {
        format!("{}-{}-{}", kind, slug, ts)
    }
}

/// Best-effort display label from a URL (its host, else the whole URL).
pub fn host_of(url: &str) -> String {
    let no_scheme = url.split("://").nth(1).unwrap_or(url);
    no_scheme
        .split('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(url)
        .to_string()
}

// ── Source management (subscribe / local / refresh / toggle / remove) ──────
//
// These orchestrate the primitives above over `settings.sources`, and are shared
// by both front-ends. The desktop's native file-picker command stays desktop-side
// and calls `add_local` with the chosen path.

use crate::registry::{builtin_registry, DISCOVERED_ID, MANUAL_ID};
use crate::scanner::{collapse_home, expand_tilde};
use crate::settings::{load_settings, mutate_settings};
use serde::Serialize;

const CURATED_SOURCE_NAME: &str = "Mux 精选";
const LEGACY_CURATED_SOURCE_NAME: &str = "官方精选合集";

/// A source as shown in a UI: its stored definition plus a live server count.
#[derive(Serialize)]
pub struct SourceView {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub format: String,
    pub enabled: bool,
    pub added_at: Option<String>,
    pub synced_at: Option<String>,
    pub server_count: u32,
    pub error: Option<String>,
    /// True for the two auto-managed sources ("手动添加" / "自动探索") — the UI
    /// hides refresh/remove for these to avoid accidental data loss.
    pub managed: bool,
}

fn to_view(def: SourceDef, count: u32) -> SourceView {
    let managed = def.id == MANUAL_ID || def.id == DISCOVERED_ID;
    let name = if def.name == LEGACY_CURATED_SOURCE_NAME {
        CURATED_SOURCE_NAME.to_string()
    } else {
        def.name
    };
    SourceView {
        id: def.id, kind: def.kind, name, url: def.url, path: def.path,
        format: def.format, enabled: def.enabled, added_at: def.added_at,
        synced_at: def.synced_at, server_count: count, error: def.error, managed,
    }
}

fn push_source(def: &SourceDef) -> Result<(), String> {
    mutate_settings(|s| s.sources.get_or_insert_with(Vec::new).push(def.clone()))
        .map_err(|e| e.to_string())
}

/// List every source with a live `server_count`.
pub fn list_views() -> Vec<SourceView> {
    load_settings()
        .sources
        .unwrap_or_default()
        .into_iter()
        .map(|d| {
            let count = source_count(&d);
            to_view(d, count)
        })
        .collect()
}

/// Subscribe to a remote config URL: fetch, validate, cache it under
/// `~/.mux/sources/remote/<id>`, and register it as an enabled source.
pub fn subscribe(url: String, name: Option<String>) -> Result<SourceView, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("URL 不能为空".into());
    }
    let body = fetch(&url)?;
    let format = detect_format(&url, &body).to_string();
    validate_parseable(&body, &format)?;
    let display = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| host_of(&url));
    let now = now_iso();
    let mut def = SourceDef::new_remote(gen_id("remote", &display), display, url, format, now);
    let path = cached_path(&def).ok_or("无法确定缓存路径")?;
    write_source_file(&path, &body)?;
    let count = source_count(&def);
    def.server_count = Some(count);
    if count == 0 {
        def.error = Some("未在该文件中发现 MCP server".into());
    }
    push_source(&def)?;
    Ok(to_view(def, count))
}

/// Register a local config file as a source: read it, validate, and copy it under
/// `~/.mux/sources/local/<id>` (the app then reads the copy, not the original).
pub fn add_local(path: String, name: Option<String>) -> Result<SourceView, String> {
    let src = expand_tilde(&path);
    let content = fs::read_to_string(&src).map_err(|e| format!("读取文件失败: {}", e))?;
    let format = detect_format(&path, &content).to_string();
    validate_parseable(&content, &format)?;
    let display = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            Path::new(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("本地配置")
                .to_string()
        });
    let now = now_iso();
    let mut def = SourceDef::new_local(
        gen_id("local", &display),
        display,
        Some(collapse_home(&path)),
        format,
        now,
    );
    let cache = cached_path(&def).ok_or("无法确定缓存路径")?;
    write_source_file(&cache, &content)?;
    let count = source_count(&def);
    def.server_count = Some(count);
    if count == 0 {
        def.error = Some("未在该文件中发现 MCP server".into());
    }
    push_source(&def)?;
    Ok(to_view(def, count))
}

/// Add the bundled curated collection as an opt-in *local* source (not part of
/// the default catalog). Serializes the embedded `data/registry.json`.
pub fn add_official() -> Result<SourceView, String> {
    let entries = builtin_registry();
    let content = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    let now = now_iso();
    let mut def = SourceDef::new_local(
        gen_id("local", "curated"),
        CURATED_SOURCE_NAME.into(),
        None,
        "json".into(),
        now,
    );
    let cache = cached_path(&def).ok_or("无法确定缓存路径")?;
    write_source_file(&cache, &content)?;
    let count = source_count(&def);
    def.server_count = Some(count);
    push_source(&def)?;
    Ok(to_view(def, count))
}

/// Re-fetch (remote) or re-copy (local) a source's file and update its status.
pub fn refresh(id: String) -> Result<SourceView, String> {
    let Some(mut def) = load_settings()
        .sources
        .unwrap_or_default()
        .into_iter()
        .find(|d| d.id == id)
    else {
        return Err("source 不存在".into());
    };
    let fetched: Result<String, String> = match def.kind.as_str() {
        "remote" => {
            let url = def.url.clone().ok_or("该来源缺少 URL")?;
            fetch(&url)
        }
        "local" => match def.path.as_ref() {
            Some(p) => {
                let src = expand_tilde(p);
                fs::read_to_string(&src).map_err(|e| format!("读取原文件失败: {}", e))
            }
            None => Err("该本地来源没有可刷新的原文件".into()),
        },
        _ => Err("不支持刷新该来源".into()),
    };
    match fetched {
        Ok(body) => {
            if let Some(path) = cached_path(&def) {
                write_source_file(&path, &body)?;
            }
            def.synced_at = Some(now_iso());
            def.error = None;
        }
        Err(e) => {
            def.error = Some(e);
        }
    }
    let count = source_count(&def);
    def.server_count = Some(count);
    let saved = def.clone();
    mutate_settings(move |s| {
        if let Some(list) = s.sources.as_mut() {
            for d in list.iter_mut() {
                if d.id == saved.id {
                    *d = saved.clone();
                }
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(to_view(def, count))
}

/// Enable or disable a source (its servers join/leave the catalog).
pub fn set_enabled(id: String, enabled: bool) -> Result<(), String> {
    mutate_settings(|s| {
        if let Some(list) = s.sources.as_mut() {
            for d in list.iter_mut() {
                if d.id == id {
                    d.enabled = enabled;
                }
            }
        }
    })
    .map_err(|e| e.to_string())
}

/// Remove a source and delete its cached file.
pub fn remove(id: String) -> Result<(), String> {
    mutate_settings(|s| {
        if let Some(list) = s.sources.as_mut() {
            if let Some(pos) = list.iter().position(|d| d.id == id) {
                let def = list.remove(pos);
                if let Some(p) = cached_path(&def) {
                    let _ = fs::remove_file(p);
                }
            }
        }
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, ext: &str) -> PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("mux-src-{}-{}.{}", name, std::process::id(), ext));
        d
    }

    fn origin() -> RegistryOrigin {
        RegistryOrigin { kind: "local".into(), agent: None, scope: None, source: Some("s1".into()) }
    }

    #[test]
    fn parses_standard_mcpservers_map() {
        let p = tmp("map", "json");
        fs::write(
            &p,
            r#"{"mcpServers":{
                "git":{"command":"npx","args":["-y","git-mcp"]},
                "wiki":{"url":"https://deepwiki.example/mcp","type":"http"}
            }}"#,
        )
        .unwrap();
        let mut entries = parse_file(&p, "json", "mcpServers", &origin());
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "git");
        assert_eq!(entries[0].transport(), "stdio");
        assert_eq!(entries[0].origin.as_ref().unwrap().kind, "local");
        assert_eq!(entries[1].name, "wiki");
        assert_eq!(entries[1].transport(), "http");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn parses_rich_registry_array_keeping_meta() {
        let p = tmp("arr", "json");
        fs::write(
            &p,
            r#"[{"name":"filesystem","description":"local fs","tags":["files"],
                "config":{"stdio":{"command":"npx","args":["-y","fs"]}}}]"#,
        )
        .unwrap();
        let entries = parse_file(&p, "json", "mcpServers", &origin());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].description, "local fs"); // rich meta preserved
        assert_eq!(entries[0].tags, vec!["files".to_string()]);
        assert_eq!(entries[0].origin.as_ref().unwrap().source.as_deref(), Some("s1"));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn detect_and_validate() {
        assert_eq!(detect_format("https://x/y.toml", "irrelevant"), "toml");
        assert_eq!(detect_format("nohint", "{\"a\":1}"), "json");
        assert!(validate_parseable("{\"a\":1}", "json").is_ok());
        assert!(validate_parseable("not json", "json").is_err());
        assert_eq!(host_of("https://raw.githubusercontent.com/x/y/main/f.json"), "raw.githubusercontent.com");
    }

    #[test]
    fn cached_path_by_kind() {
        let mut d = SourceDef {
            id: "abc".into(), kind: "remote".into(), name: "n".into(), url: None, path: None,
            format: "json".into(), key: "mcpServers".into(), enabled: true,
            added_at: None, synced_at: None, server_count: None, error: None,
        };
        assert!(cached_path(&d).unwrap().ends_with("remote/abc.json"));
        d.kind = "local".into();
        d.format = "toml".into();
        assert!(cached_path(&d).unwrap().ends_with("local/abc.toml"));
    }

    #[test]
    fn legacy_curated_source_name_is_normalized() {
        let def = SourceDef {
            id: "curated".into(),
            kind: "remote".into(),
            name: LEGACY_CURATED_SOURCE_NAME.into(),
            url: None,
            path: None,
            format: "json".into(),
            key: "mcpServers".into(),
            enabled: true,
            added_at: None,
            synced_at: None,
            server_count: None,
            error: None,
        };

        assert_eq!(to_view(def, 0).name, CURATED_SOURCE_NAME);
    }
}
