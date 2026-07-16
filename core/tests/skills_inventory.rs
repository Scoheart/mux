#![cfg(unix)]

mod support;

use mux_core::settings::mutate_settings;
use mux_core::skills::{
    get_skill_detail, hash_tree, list_inventory, list_skill_agents, normalize_agent_selection,
    InventoryState, SkillError,
};
use mux_core::testenv::TestHome;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use support::skills::{has_state, managed_record, write_skill};

fn install_cursor(home: &std::path::Path) {
    fs::create_dir_all(home.join("Library/Application Support/Cursor")).unwrap();
}

#[test]
fn only_installed_verified_agents_are_assignable_and_aliases_expand_impact() {
    let th = TestHome::new("skills-targets");
    fs::create_dir_all(th.home.join(".codex")).unwrap();
    install_cursor(&th.home);
    fs::create_dir_all(th.home.join(".config/opencode")).unwrap();

    let agents = list_skill_agents().unwrap();
    assert_eq!(
        agents.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        vec!["codex", "cursor", "opencode"]
    );
    let codex = agents.iter().find(|row| row.id == "codex").unwrap();
    assert_eq!(codex.target_id, "agents-user");
    assert_eq!(
        codex.affected_agent_ids,
        vec!["codex", "cursor", "opencode"]
    );

    assert_eq!(
        normalize_agent_selection(&["codex".into(), "cursor".into()]).unwrap(),
        vec!["agents-user"]
    );
}

#[test]
fn inventory_distinguishes_external_broken_conflicting_and_modified() {
    let th = TestHome::new("inventory-states");
    fs::create_dir_all(th.home.join(".codex")).unwrap();
    install_cursor(&th.home);
    let central = th.home.join(".mux/skills/managed");
    write_skill(&central, "managed", "Managed fixture");
    let original_hash = hash_tree(&central).unwrap();
    mutate_settings(|settings| {
        settings
            .managed_skills
            .get_or_insert_default()
            .insert("managed".into(), managed_record("managed", &original_hash));
    })
    .unwrap();
    fs::write(
        central.join("SKILL.md"),
        "---\nname: managed\ndescription: Changed fixture\n---\n",
    )
    .unwrap();

    write_skill(
        &th.home.join(".agents/skills/external"),
        "external",
        "External fixture",
    );
    fs::create_dir_all(th.home.join(".cursor/skills")).unwrap();
    symlink(
        th.home.join("missing/broken"),
        th.home.join(".cursor/skills/broken"),
    )
    .unwrap();
    let wrong = th.home.join("wrong/conflict");
    write_skill(&wrong, "conflict", "Wrong target fixture");
    symlink(&wrong, th.home.join(".cursor/skills/conflict")).unwrap();

    let inventory = list_inventory().unwrap();
    assert!(has_state(&inventory, "external", InventoryState::External));
    assert!(has_state(&inventory, "broken", InventoryState::BrokenLink));
    assert!(has_state(
        &inventory,
        "conflict",
        InventoryState::ConflictingLink
    ));
    assert!(has_state(
        &inventory,
        "managed",
        InventoryState::LocallyModified
    ));
}

#[test]
fn assigned_target_remains_visible_after_its_agent_probe_disappears() {
    let th = TestHome::new("inventory-orphaned-target");
    let central = th.home.join(".mux/skills/safe");
    write_skill(&central, "safe", "Managed fixture");
    let hash = hash_tree(&central).unwrap();
    mutate_settings(|settings| {
        settings
            .managed_skills
            .get_or_insert_default()
            .insert("safe".into(), managed_record("safe", &hash));
        settings
            .skill_assignments
            .get_or_insert_default()
            .insert("safe".into(), ["cursor-user".into()].into_iter().collect());
    })
    .unwrap();
    let target = th.home.join(".cursor/skills/safe");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    symlink(&central, &target).unwrap();

    let inventory = list_inventory().unwrap();
    assert!(inventory.agents.iter().all(|agent| agent.id != "cursor"));
    assert!(has_state(&inventory, "safe", InventoryState::Assigned));
    assert!(
        !inventory
            .targets
            .iter()
            .find(|row| row.target_id == "cursor-user")
            .unwrap()
            .assignable
    );
}

#[test]
fn command_probes_require_executable_regular_files_in_disposable_roots() {
    let th = TestHome::new("inventory-command-probes");
    let fallback = th.home.join("opt/homebrew/bin");
    fs::create_dir_all(&fallback).unwrap();
    let gemini = fallback.join("gemini");
    fs::write(&gemini, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&gemini, fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir(fallback.join("claude")).unwrap();
    fs::write(fallback.join("copilot"), "not executable").unwrap();

    let agents = list_skill_agents().unwrap();
    assert!(agents.iter().any(|agent| agent.id == "gemini"));
    assert!(agents.iter().all(|agent| agent.id != "claude-code"));
    assert!(agents.iter().all(|agent| agent.id != "copilot-cli"));

    let executable = th.home.join("fixture-copilot");
    fs::write(&executable, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let usr_local = th.home.join("usr/local/bin");
    fs::create_dir_all(&usr_local).unwrap();
    symlink(&executable, usr_local.join("copilot")).unwrap();
    assert!(list_skill_agents()
        .unwrap()
        .iter()
        .any(|agent| agent.id == "copilot-cli"));
}

#[test]
fn installed_alias_only_target_is_scanned_but_not_assignable() {
    let th = TestHome::new("inventory-alias-only-target");
    install_cursor(&th.home);
    write_skill(
        &th.home.join(".agents/skills/shared"),
        "shared",
        "Shared alias fixture",
    );

    let inventory = list_inventory().unwrap();
    assert_eq!(
        inventory
            .agents
            .iter()
            .map(|agent| agent.id.as_str())
            .collect::<Vec<_>>(),
        vec!["cursor"]
    );
    let target = inventory
        .targets
        .iter()
        .find(|target| target.target_id == "agents-user")
        .unwrap();
    assert_eq!(target.affected_agent_ids, vec!["cursor"]);
    assert!(!target.assignable);
    assert!(has_state(&inventory, "shared", InventoryState::External));
}

#[test]
fn canonical_target_graph_rejects_two_ids_resolving_to_one_path() {
    let th = TestHome::new("inventory-target-collision");
    fs::create_dir_all(th.home.join(".agents/skills")).unwrap();
    fs::create_dir_all(th.home.join(".codex")).unwrap();
    install_cursor(&th.home);
    symlink(th.home.join(".agents"), th.home.join(".cursor")).unwrap();

    assert!(matches!(
        list_skill_agents(),
        Err(SkillError::InvalidSource { .. })
    ));
}

#[test]
fn canonical_target_graph_rejects_a_non_directory_existing_parent() {
    let th = TestHome::new("inventory-target-file-parent");
    fs::create_dir_all(th.home.join(".codex")).unwrap();
    fs::create_dir_all(th.home.join(".agents")).unwrap();
    fs::write(th.home.join(".agents/skills"), "not a directory").unwrap();

    assert!(matches!(
        list_skill_agents(),
        Err(SkillError::InvalidSource { .. })
    ));
}

#[test]
fn selection_rejects_duplicates_unknown_ids_and_agents_without_current_evidence() {
    let th = TestHome::new("inventory-invalid-selection");
    fs::create_dir_all(th.home.join(".codex")).unwrap();

    assert!(normalize_agent_selection(&["codex".into(), "codex".into()]).is_err());
    assert!(normalize_agent_selection(&["not-an-agent".into()]).is_err());
    assert!(normalize_agent_selection(&["cursor".into()]).is_err());
}

#[test]
fn detail_uses_opaque_identity_and_truncates_skill_md_at_a_utf8_boundary() {
    let th = TestHome::new("inventory-detail-bound");
    let central = th.home.join(".mux/skills/large");
    fs::create_dir_all(&central).unwrap();
    let mut skill_md = "---\nname: large\ndescription: Large fixture\n---\n\n".to_string();
    skill_md.push_str(&"界".repeat(400_000));
    fs::write(central.join("SKILL.md"), skill_md).unwrap();
    let hash = hash_tree(&central).unwrap();
    mutate_settings(|settings| {
        settings
            .managed_skills
            .get_or_insert_default()
            .insert("large".into(), managed_record("large", &hash));
    })
    .unwrap();

    let inventory = list_inventory().unwrap();
    let item = inventory
        .items
        .iter()
        .find(|item| item.name == "large")
        .unwrap();
    assert_eq!(item.identity, "central:large");
    let detail = get_skill_detail(&item.identity).unwrap();
    assert!(detail.skill_md.len() <= 1024 * 1024);
    assert!(detail.skill_md_truncated);
    assert!(!detail.files.is_empty());
    assert!(std::str::from_utf8(detail.skill_md.as_bytes()).is_ok());

    for invalid in [
        "central:../large",
        "central:/tmp/large",
        "target:not-known:large",
        "target:cursor-user:/tmp/large",
    ] {
        assert!(get_skill_detail(invalid).is_err(), "accepted {invalid}");
    }
}

#[test]
fn strict_settings_errors_do_not_expose_private_paths() {
    let th = TestHome::new("inventory-corrupt-settings");
    fs::create_dir_all(th.home.join(".mux")).unwrap();
    fs::write(
        th.home.join(".mux/settings.json"),
        r#"{"managed_skills": ["#,
    )
    .unwrap();

    let error = list_inventory().unwrap_err();
    let rendered = serde_json::to_string(&error).unwrap();
    assert!(!rendered.contains(th.home.to_string_lossy().as_ref()));
    assert!(!rendered.contains("settings.json"));
}

#[test]
fn read_only_inventory_does_not_create_mux_skill_roots() {
    let th = TestHome::new("inventory-read-only-roots");
    let mux_home = th.home.join(".mux");

    let inventory = list_inventory().unwrap();

    assert!(inventory.items.is_empty());
    assert!(!mux_home.exists(), "a list API created the MUX home");
}

#[test]
fn missing_installed_target_roots_remain_absent_after_listing() {
    let th = TestHome::new("inventory-missing-target-roots");
    install_cursor(&th.home);
    let target = th.home.join(".cursor/skills");

    let inventory = list_inventory().unwrap();

    assert!(inventory.items.is_empty());
    assert!(
        !target.exists(),
        "a list API created a missing Agent target"
    );
    assert!(!th.home.join(".mux").exists());
}

#[test]
fn invalid_managed_tree_is_visible_but_is_not_marked_managed() {
    let th = TestHome::new("inventory-corrupt-managed-tree");
    let central = th.home.join(".mux/skills/corrupt");
    write_skill(&central, "wrong-name", "Mismatched manifest fixture");
    let hash = hash_tree(&central).unwrap();
    mutate_settings(|settings| {
        settings
            .managed_skills
            .get_or_insert_default()
            .insert("corrupt".into(), managed_record("corrupt", &hash));
    })
    .unwrap();

    let inventory = list_inventory().unwrap();
    let item = inventory
        .items
        .iter()
        .find(|item| item.identity == "central:corrupt")
        .unwrap();
    assert!(item.states.contains(&InventoryState::LocallyModified));
    assert!(!item.states.contains(&InventoryState::Managed));
    assert_eq!(item.content_hash.as_deref(), Some(hash.as_str()));
}

#[test]
fn managed_central_link_failures_are_broken_and_never_managed() {
    let th = TestHome::new("inventory-managed-central-links");
    let central_root = th.home.join(".mux/skills");
    fs::create_dir_all(&central_root).unwrap();
    symlink(
        th.home.join("missing/dangling"),
        central_root.join("dangling"),
    )
    .unwrap();
    symlink("loop", central_root.join("loop")).unwrap();
    let existing = th.home.join("existing-link-target");
    fs::create_dir(&existing).unwrap();
    symlink(&existing, central_root.join("resolving")).unwrap();
    mutate_settings(|settings| {
        let records = settings.managed_skills.get_or_insert_default();
        records.insert("dangling".into(), managed_record("dangling", "recorded"));
        records.insert("loop".into(), managed_record("loop", "recorded"));
        records.insert("resolving".into(), managed_record("resolving", "recorded"));
    })
    .unwrap();

    let inventory = list_inventory().unwrap();
    for name in ["dangling", "loop"] {
        let item = inventory
            .items
            .iter()
            .find(|item| item.identity == format!("central:{name}"))
            .unwrap();
        assert!(item.states.contains(&InventoryState::BrokenLink));
        assert!(!item.states.contains(&InventoryState::Managed));
    }
    let resolving = inventory
        .items
        .iter()
        .find(|item| item.identity == "central:resolving")
        .unwrap();
    assert!(resolving.states.contains(&InventoryState::ConflictingLink));
    assert!(!resolving.states.contains(&InventoryState::Managed));
}

#[test]
fn target_self_loop_is_reported_as_broken_instead_of_aborting_inventory() {
    let th = TestHome::new("inventory-target-loop");
    install_cursor(&th.home);
    let target_root = th.home.join(".cursor/skills");
    fs::create_dir_all(&target_root).unwrap();
    symlink("loop", target_root.join("loop")).unwrap();

    let inventory = list_inventory().unwrap();
    let item = inventory
        .items
        .iter()
        .find(|item| item.identity == "target:cursor-user:loop")
        .unwrap();
    assert!(item.states.contains(&InventoryState::BrokenLink));
}

#[test]
fn inventory_root_errors_do_not_expose_the_private_home() {
    let th = TestHome::new("inventory-private-root-error");
    fs::create_dir_all(th.home.join(".mux")).unwrap();
    fs::write(th.home.join(".mux/skills"), "not a directory").unwrap();

    let error = list_inventory().unwrap_err();
    let rendered = serde_json::to_string(&error).unwrap();
    assert!(!rendered.contains(th.home.to_string_lossy().as_ref()));
    assert!(!rendered.contains("skills"));
}

#[test]
fn read_api_path_resolution_errors_are_path_free() {
    let th = TestHome::new("inventory-path-resolution-error");
    std::env::set_var("MUX_HOME", "relative-mux-home");

    let error = list_inventory().unwrap_err();
    let rendered = serde_json::to_string(&error).unwrap();
    assert!(matches!(error, SkillError::InvalidSource { .. }));
    assert!(!rendered.contains(th.home.to_string_lossy().as_ref()));
    assert!(!rendered.contains("settings.json"));
}

#[test]
fn external_list_scan_reads_only_the_bounded_manifest_but_detail_walks_the_tree() {
    let th = TestHome::new("inventory-minimal-external-scan");
    fs::create_dir_all(th.home.join(".codex")).unwrap();
    let root = th.home.join(".agents/skills/external");
    write_skill(&root, "external", "Bounded external summary");
    symlink("../../../../outside", root.join("escape")).unwrap();

    let inventory = list_inventory().unwrap();
    let item = inventory
        .items
        .iter()
        .find(|item| item.identity == "target:agents-user:external")
        .unwrap();
    assert_eq!(item.description, "Bounded external summary");

    let error = get_skill_detail(&item.identity).unwrap_err();
    let rendered = serde_json::to_string(&error).unwrap();
    assert!(!rendered.contains(th.home.to_string_lossy().as_ref()));
}

#[test]
fn unreadable_target_is_a_hard_path_free_error_not_a_missing_inventory() {
    let th = TestHome::new("inventory-unreadable-target");
    install_cursor(&th.home);
    let target = th.home.join(".cursor/skills");
    fs::create_dir_all(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o000)).unwrap();

    let result = list_inventory();

    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    let error = result.expect_err("an unreadable target must not be treated as empty");
    let rendered = serde_json::to_string(&error).unwrap();
    assert!(matches!(
        error,
        SkillError::Io { .. } | SkillError::Conflict { .. }
    ));
    assert!(!rendered.contains(th.home.to_string_lossy().as_ref()));
}
