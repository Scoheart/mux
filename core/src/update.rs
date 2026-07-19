//! CLI 自更新：对着 GitHub 稳定 Release 通道(最新 vX.Y.Z)检查/替换 `mux` 二进制。
//! 桌面端的自更新由 tauri-plugin-updater 走同一通道(latest.json)，不经过这里。

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crate::paths::mux_dir;

const REPO: &str = "scoheart/mux";
/// 被动检查的间隔：每天最多打一次 GitHub API。
const PASSIVE_CHECK_INTERVAL_SECS: i64 = 24 * 60 * 60;

fn api_agent() -> Result<ureq::Agent, String> {
    crate::network::build_ureq_agent(
        ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(15))
            // GitHub API 要求带 User-Agent
            .user_agent("mux-cli-updater"),
    )
}

/// 当前运行的 `mux` 由桌面 App 提供时(真身在某个 `.app` 包内——直接运行包内
/// 二进制，或经 `~/.local/bin/mux` 软链)，返回真身路径。这种安装随桌面 App
/// 自动更新，`upgrade` 不应自行替换(会破坏 .app 的签名/更新一致性)。
pub fn managed_by_desktop_app() -> Option<PathBuf> {
    let real = std::env::current_exe().ok()?.canonicalize().ok()?;
    real.components()
        .any(|c| c.as_os_str().to_string_lossy().ends_with(".app"))
        .then_some(real)
}

/// 查询最新稳定 Release 的版本号(去掉 `v` 前缀)。预发布(-build.N)不会出现在
/// `releases/latest`，所以这里天然只追正式版通道。
pub fn fetch_latest_version() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let body = api_agent()?
        .get(&url)
        .call()
        .map_err(|e| format!("查询最新版本失败: {}", e))?
        .into_string()
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析响应失败: {}", e))?;
    let tag = json["tag_name"]
        .as_str()
        .ok_or("响应缺少 tag_name(可能还没有正式 Release)")?;
    Ok(tag.trim_start_matches('v').to_string())
}

/// 简单的点分数字版本比较：`latest` 是否比 `current` 新。
/// 非数字段按 0 处理；长度不齐则短的补 0。
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|s| {
                s.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect()
    };
    let (l, c) = (parse(latest), parse(current));
    let n = l.len().max(c.len());
    for i in 0..n {
        let (a, b) = (
            l.get(i).copied().unwrap_or(0),
            c.get(i).copied().unwrap_or(0),
        );
        if a != b {
            return a > b;
        }
    }
    false
}

fn target_triple() -> String {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        other => other,
    };
    let os = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        other => other,
    };
    format!("{}-{}", arch, os)
}

pub struct UpgradeOutcome {
    pub from: String,
    pub to: String,
}

/// 把当前运行的 `mux` 替换为最新 Release 里的二进制。
/// 已是最新时返回 `Ok(None)`。
pub fn upgrade_cli(current_version: &str) -> Result<Option<UpgradeOutcome>, String> {
    let latest = fetch_latest_version()?;
    if !is_newer(&latest, current_version) {
        return Ok(None);
    }

    // Release 资产名与 CI 打包一致：mux_v{ver}_{triple}.tar.gz
    let triple = target_triple();
    let asset = format!("mux_v{}_{}.tar.gz", latest, triple);
    let url = format!(
        "https://github.com/{}/releases/download/v{}/{}",
        REPO, latest, asset
    );

    // 下载到 ~/.mux 下的临时目录(与最终 rename 同卷，且天然可写)。
    let tmp_dir = mux_dir().join("update-tmp");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
    let tarball = tmp_dir.join(&asset);

    let resp = api_agent()?
        .get(&url)
        .call()
        .map_err(|e| format!("下载 {} 失败: {}", asset, e))?;
    let mut reader = resp.into_reader();
    let mut file = fs::File::create(&tarball).map_err(|e| format!("写入临时文件失败: {}", e))?;
    std::io::copy(&mut reader, &mut file).map_err(|e| format!("下载中断: {}", e))?;
    drop(file);

    // 解包(依赖系统 tar，macOS/Linux 都有，免拉压缩库依赖)。
    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&tarball)
        .arg("-C")
        .arg(&tmp_dir)
        .status()
        .map_err(|e| format!("调用 tar 失败: {}", e))?;
    if !status.success() {
        return Err("解包更新失败".into());
    }
    let new_bin = tmp_dir.join("mux");
    if !new_bin.exists() {
        return Err("更新包里没有 mux 二进制".into());
    }

    // 原子替换：老的先挪走(运行中的二进制可以 rename)，新的挪进来，失败则回滚。
    let current_exe = std::env::current_exe().map_err(|e| format!("定位当前二进制失败: {}", e))?;
    let backup: PathBuf = current_exe.with_extension("old");
    let _ = fs::remove_file(&backup);
    fs::rename(&current_exe, &backup).map_err(|e| format!("备份当前二进制失败: {}", e))?;
    if let Err(e) =
        fs::rename(&new_bin, &current_exe).or_else(|_| fs::copy(&new_bin, &current_exe).map(|_| ()))
    {
        let _ = fs::rename(&backup, &current_exe); // 回滚
        return Err(format!("替换二进制失败: {}", e));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&current_exe, fs::Permissions::from_mode(0o755));
    }
    let _ = fs::remove_file(&backup);
    let _ = fs::remove_dir_all(&tmp_dir);

    Ok(Some(UpgradeOutcome {
        from: current_version.to_string(),
        to: latest,
    }))
}

/// 被动更新提醒：普通命令跑完后调用。每天最多联网查一次(结果缓存在
/// `~/.mux/update-check.json`)，有新版本时返回一行提示文案。
/// 设置 `MUX_NO_UPDATE_CHECK=1` 可完全关闭。
pub fn passive_check_notice(current_version: &str) -> Option<String> {
    if std::env::var_os("MUX_NO_UPDATE_CHECK").is_some() {
        return None;
    }
    // 桌面 App 带出来的 CLI 随 App 更新——提示 `mux upgrade` 没有意义。
    if managed_by_desktop_app().is_some() {
        return None;
    }
    let cache_file = mux_dir().join("update-check.json");
    let now = chrono::Utc::now().timestamp();

    // 读缓存：一天内不重复联网，直接用上次的结果。
    let cached: Option<serde_json::Value> = fs::read_to_string(&cache_file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let latest = match &cached {
        Some(c) if now - c["checked_at"].as_i64().unwrap_or(0) < PASSIVE_CHECK_INTERVAL_SECS => {
            c["latest"].as_str().map(str::to_string)
        }
        _ => {
            // 缓存过期才联网；失败也写缓存，避免离线时每条命令都卡一次超时。
            let fetched = fetch_latest_version().ok();
            let _ = fs::create_dir_all(mux_dir());
            let _ = fs::write(
                &cache_file,
                serde_json::json!({
                    "checked_at": now,
                    "latest": fetched,
                })
                .to_string(),
            );
            fetched
        }
    }?;

    if is_newer(&latest, current_version) {
        Some(format!(
            "✨ 新版本 v{} 可用(当前 v{})，运行 `mux upgrade` 升级",
            latest, current_version
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_version_compare() {
        assert!(is_newer("0.2.0", "0.1.2"));
        assert!(is_newer("0.1.10", "0.1.2"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.2", "0.1.2"));
        assert!(!is_newer("0.1.1", "0.1.2"));
        // 长度不齐、非纯数字段
        assert!(is_newer("0.1.2.1", "0.1.2"));
        assert!(!is_newer("0.1", "0.1.0"));
    }

    #[test]
    fn triple_matches_ci_naming() {
        let t = target_triple();
        assert!(t.contains('-'));
    }
}
