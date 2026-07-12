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

/// `~/.mux/sources` —— 用户来源(订阅/本地)缓存文件的根目录
pub fn sources_dir() -> PathBuf {
    mux_dir().join("sources")
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mux_dir_ends_with_dot_mux() {
        assert!(mux_dir().ends_with(".mux"));
    }
    #[test]
    fn backups_under_mux_dir() {
        assert!(backups_dir().starts_with(mux_dir()));
    }
}

/// Filename-safe local timestamp (`%Y-%m-%dT%H-%M-%S`) used for backup artifacts.
pub fn backup_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}
