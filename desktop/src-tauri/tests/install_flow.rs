// 端到端集成测试：headless 真实跑安装流程，验证配置文件被正确写入、覆写生效、备份生成。
// 用项目级 scope（写入临时目录），HOME 指向临时目录使备份/agent 解析保持隔离。
use std::collections::HashMap;
use std::fs;

use desktop_lib::commands::{apply_install, preview_install, InstallRequest, PatchInput};

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mux-e2e-{}-{}", tag, std::process::id()));
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn install_flow_writes_config_applies_override_and_backs_up() {
    // 隔离 HOME/MUX_HOME（备份目录 ~/.mux/backups 与 agent 解析都基于 home）
    let th = mux_core::testenv::TestHome::new("install");
    let home = th.home.clone();

    // No built-in catalog anymore — seed a `filesystem` entry through the real
    // store (a managed local source) so the install flow has something to resolve.
    mux_core::registry::write_manual_entry(&mux_core::types::RegistryEntry {
        name: "filesystem".into(),
        description: String::new(),
        tags: vec![],
        config: mux_core::types::RegistryConfig {
            stdio: Some(mux_core::types::StdioConfig {
                command: "npx".into(),
                args: Some(vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into(), ".".into()]),
                env: None,
            }),
            http: None,
        },
        origin: None,
        repo: None,
    })
    .unwrap();

    let project = unique_dir("proj");

    // —— 1) 预览：filesystem 装到 claude-code 的项目配置（.mcp.json）——
    let req = InstallRequest {
        server_name: "filesystem".into(),
        transport: "stdio".into(),
        scope: "project".into(),
        agents: vec!["claude-code".into()],
        project_dir: Some(project.display().to_string()),
        overrides: HashMap::new(),
    };
    let plan = preview_install(req).expect("preview should succeed");
    assert_eq!(plan.len(), 1, "应有 1 条计划写入");
    assert_eq!(plan[0].agent, "claude-code");
    assert!(plan[0].file_path.ends_with(".mcp.json"), "目标应为 .mcp.json，实际 {}", plan[0].file_path);
    assert!(plan[0].config_json.contains("command"), "预览配置应含 command");

    // —— 2) 应用：真实写入文件 ——
    let req2 = InstallRequest {
        server_name: "filesystem".into(),
        transport: "stdio".into(),
        scope: "project".into(),
        agents: vec!["claude-code".into()],
        project_dir: Some(project.display().to_string()),
        overrides: HashMap::new(),
    };
    apply_install(req2).expect("apply should succeed");

    let mcp_file = project.join(".mcp.json");
    let written = fs::read_to_string(&mcp_file).expect(".mcp.json 应已写入");
    assert!(written.contains("mcpServers"), "应含 mcpServers 键");
    assert!(written.contains("filesystem"), "应含 filesystem 服务器");
    let v: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert!(v["mcpServers"]["filesystem"]["command"].is_string(), "filesystem.command 应存在");

    // —— 3) 逐 agent 覆写：给 claude-code 加 env，再应用（此时文件已存在 → 触发备份）——
    let mut env = HashMap::new();
    env.insert("MY_TOKEN".to_string(), "secret-abc".to_string());
    let mut overrides = HashMap::new();
    overrides.insert(
        "claude-code".to_string(),
        PatchInput { args: None, env: Some(env), url: None, headers: None },
    );
    let req3 = InstallRequest {
        server_name: "filesystem".into(),
        transport: "stdio".into(),
        scope: "project".into(),
        agents: vec!["claude-code".into()],
        project_dir: Some(project.display().to_string()),
        overrides,
    };
    apply_install(req3).expect("apply with override should succeed");

    let written2 = fs::read_to_string(&mcp_file).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&written2).unwrap();
    assert_eq!(
        v2["mcpServers"]["filesystem"]["env"]["MY_TOKEN"], "secret-abc",
        "覆写的 env 应写入该 agent 的配置"
    );

    // —— 4) 备份：第二次应用前文件已存在，应在 ~/.mux/backups 生成带时间戳的备份 ——
    let backups = home.join(".mux").join("backups");
    let has_backup = backups.exists()
        && fs::read_dir(&backups).unwrap().any(|e| {
            e.unwrap().file_name().to_string_lossy().starts_with(".mcp.json-")
        });
    assert!(has_backup, "写入前应生成 .mcp.json 的备份，目录 {:?}", backups);

    // 直观打印（--nocapture 时可见）
    println!("\n===== 实际写入的 {} =====\n{}", mcp_file.display(), written2);
    let backup_names: Vec<String> = fs::read_dir(&backups).unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    println!("===== {} 下的备份 =====\n{:?}\n", backups.display(), backup_names);

    // 清理（home 由 TestHome Drop 负责）
    let _ = fs::remove_dir_all(&project);
}
