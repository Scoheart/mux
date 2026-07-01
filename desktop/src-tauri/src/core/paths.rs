use std::path::PathBuf;

/// `~/.mux` —— 与 CLI 共用的数据目录
pub fn mux_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".mux")
}

pub fn backups_dir() -> PathBuf {
    mux_dir().join("backups")
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
