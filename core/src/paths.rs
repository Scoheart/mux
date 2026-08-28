use std::path::PathBuf;

/// `~/.mux` —— 与 CLI 共用的数据目录。
///
/// `MUX_HOME` 环境变量可整体重定向该目录（值即数据目录本身，类似
/// `CARGO_HOME`）。除了给用户/CI 挪数据目录，它也是测试隔离的关键防线：
/// 即使测试对 `HOME` 的操纵发生竞态，只要 `MUX_HOME` 指向临时目录，
/// 真实 `~/.mux` 就不会被写脏（2026-07-08 曾因 HOME 竞态污染真实缓存）。
pub fn mux_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("MUX_HOME") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".mux")
}

pub fn backups_dir() -> PathBuf {
    mux_dir().join("backups")
}

/// `~/.mux/assets` —— MCP、Model 与 Skill 中央资产的统一物理根目录。
pub fn assets_dir() -> PathBuf {
    mux_dir().join("assets")
}

pub fn mcp_assets_dir() -> PathBuf {
    assets_dir().join("mcps")
}

pub fn mcp_sources_dir() -> PathBuf {
    mcp_assets_dir().join("sources")
}

pub fn mcp_icons_dir() -> PathBuf {
    assets_dir().join("mcp-icons")
}

pub fn model_assets_dir() -> PathBuf {
    assets_dir().join("models")
}

pub fn skill_assets_dir() -> PathBuf {
    assets_dir().join("skills")
}

pub fn skill_contents_dir() -> PathBuf {
    skill_assets_dir().join("items")
}

pub fn mcp_catalog_file() -> PathBuf {
    mcp_assets_dir().join("catalog.json")
}

pub fn model_catalog_file() -> PathBuf {
    model_assets_dir().join("catalog.json")
}

pub fn skill_catalog_file() -> PathBuf {
    skill_assets_dir().join("catalog.json")
}

pub(crate) fn legacy_sources_dir() -> PathBuf {
    mux_dir().join("sources")
}

pub(crate) fn legacy_skills_dir() -> PathBuf {
    mux_dir().join("skills")
}

/// MCP source payload root. Before the startup migration completes, readers
/// transparently resolve the legacy `~/.mux/sources` location so recovery and
/// upgrades never observe an empty catalog merely because the binary changed.
pub fn sources_dir() -> PathBuf {
    let current = mcp_sources_dir();
    let legacy = legacy_sources_dir();
    if !current.exists() && legacy.exists() {
        legacy
    } else {
        current
    }
}

/// `~/.mux/sources/remote` —— 订阅(远程 URL)抓取后的缓存副本
pub fn remote_sources_dir() -> PathBuf {
    sources_dir().join("remote")
}

/// `~/.mux/sources/local` —— 本地添加的配置文件副本
pub fn local_sources_dir() -> PathBuf {
    sources_dir().join("local")
}

/// `~/.mux/settings.json` —— 所有用户数据(registry/agents/disabled/state…)的单一文件
pub fn settings_file() -> PathBuf {
    mux_dir().join("settings.json")
}

/// `~/.mux/registry` —— legacy 自定义条目目录(仅迁移时读取)
pub fn registry_dir() -> PathBuf {
    mux_dir().join("registry")
}

pub fn user_agents_file() -> PathBuf {
    mux_dir().join("agents.json")
}

fn migrate_directory(legacy: &std::path::Path, current: &std::path::Path) -> std::io::Result<bool> {
    match (legacy.exists(), current.exists()) {
        (false, _) => Ok(false),
        (true, false) => {
            if let Some(parent) = current.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(legacy, current)?;
            Ok(true)
        }
        (true, true) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "refusing to merge legacy asset directory {} into existing {}",
                legacy.display(),
                current.display()
            ),
        )),
    }
}

#[cfg(unix)]
fn create_directory_alias(
    target: &std::path::Path,
    alias: &std::path::Path,
) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, alias)
}

#[cfg(windows)]
fn create_directory_alias(
    target: &std::path::Path,
    alias: &std::path::Path,
) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, alias)
}

/// Atomically move legacy central asset directories under `~/.mux/assets`.
/// The settings lock must be held by the caller. A simultaneous legacy and new
/// directory is ambiguous and therefore fails closed instead of merging trees.
/// The old Skill root becomes a compatibility symlink so links created by an
/// earlier MUX release keep resolving to the one physical central copy.
pub(crate) fn migrate_legacy_asset_directories() -> std::io::Result<bool> {
    let mut changed = migrate_directory(&legacy_sources_dir(), &mcp_sources_dir())?;

    let legacy = legacy_skills_dir();
    let current = skill_contents_dir();
    match std::fs::symlink_metadata(&legacy) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !current.exists() {
                return Ok(changed);
            }
        }
        Err(error) => return Err(error),
        Ok(metadata) if metadata.file_type().is_dir() => {
            migrate_directory(&legacy, &current)?;
        }
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let resolved = std::fs::canonicalize(&legacy)?;
            let expected = std::fs::canonicalize(&current)?;
            if resolved != expected {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "legacy Skill alias {} does not resolve to {}",
                        legacy.display(),
                        current.display()
                    ),
                ));
            }
            return Ok(changed);
        }
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("legacy Skill path {} is not a directory", legacy.display()),
            ));
        }
    }
    create_directory_alias(&current, &legacy)?;
    changed = true;
    Ok(changed)
}

/// Filename-safe local timestamp (`%Y-%m-%dT%H-%M-%S`) used for backup artifacts.
pub fn backup_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testenv::TestHome;

    #[test]
    fn mux_dir_ends_with_dot_mux() {
        let _home = TestHome::new("paths-mux-dir");
        assert!(mux_dir().ends_with(".mux"));
    }

    #[test]
    fn backups_under_mux_dir() {
        let _home = TestHome::new("paths-backups");
        assert!(backups_dir().starts_with(mux_dir()));
    }
}
