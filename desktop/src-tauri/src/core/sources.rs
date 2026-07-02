//! Catalog **sources**: subscribed remote URLs and added local files.
//!
//! A source's servers are parsed from a cached copy on disk under
//! `~/.mux/sources/<kind>/<id>.<ext>`. This module owns the parse / fetch /
//! store primitives; the Tauri commands in `commands.rs` orchestrate them.
//! There is no "builtin" source — the catalog is entirely user-driven.

use crate::core::adapter::get_adapter;
use crate::core::paths::{local_sources_dir, remote_sources_dir};
use crate::core::types::{McpConfig, RegistryConfig, RegistryEntry, RegistryOrigin, SourceDef};
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
    let config = match cfg {
        McpConfig::Stdio(c) => RegistryConfig { stdio: Some(c), http: None },
        McpConfig::Http(c) => RegistryConfig { stdio: None, http: Some(c) },
    };
    RegistryEntry {
        name,
        description: String::new(),
        tags: Vec::new(),
        config,
        origin: Some(origin.clone()),
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
                return arr
                    .into_iter()
                    .map(|mut e| {
                        e.origin = Some(origin.clone());
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
}
