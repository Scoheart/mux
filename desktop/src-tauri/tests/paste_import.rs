// Pasting a config blob (e.g. a `mcpServers` JSON object) adds its servers to the
// managed "manual" source. One test (single $HOME) to avoid env races.
use std::fs;

use desktop_lib::commands::{import_pasted_config, list_registry};

#[test]
fn paste_import_recognizes_and_adds_manual_entries() {
    let h = std::env::temp_dir().join(format!("mux-paste-{}", std::process::id()));
    let _ = fs::remove_dir_all(&h);
    fs::create_dir_all(&h).unwrap();
    std::env::set_var("HOME", &h);

    // 1) A standard mcpServers block (the yunxiao example).
    let text = r#"{
      "mcpServers": {
        "yunxiao": {
          "command": "npx",
          "args": ["-y", "alibabacloud-devops-mcp-server"],
          "env": { "YUNXIAO_ACCESS_TOKEN": "<YOUR_TOKEN>" }
        }
      }
    }"#;
    let added = import_pasted_config(text.into()).expect("paste import should succeed");
    assert_eq!(added, vec!["yunxiao".to_string()]);

    let e = list_registry().into_iter().find(|e| e.name == "yunxiao").expect("yunxiao present");
    assert_eq!(e.origin.as_ref().unwrap().kind, "manual");
    let stdio = e.config.stdio.as_ref().unwrap();
    assert_eq!(stdio.command, "npx");
    assert_eq!(stdio.env.as_ref().unwrap().get("YUNXIAO_ACCESS_TOKEN").unwrap(), "<YOUR_TOKEN>");
    assert!(h.join(".mux/sources/local/manual.json").exists());

    // 2) A bare name->config map (no mcpServers wrapper).
    let added = import_pasted_config(r#"{"git":{"command":"npx","args":["-y","git-mcp"]}}"#.into()).unwrap();
    assert_eq!(added, vec!["git".to_string()]);
    assert!(list_registry().iter().any(|e| e.name == "git"));

    // 3) Junk / no servers → error.
    assert!(import_pasted_config("hello world".into()).is_err());
    assert!(import_pasted_config(r#"{"foo":123}"#.into()).is_err());
    assert!(import_pasted_config("".into()).is_err());

    std::env::remove_var("HOME");
    let _ = fs::remove_dir_all(&h);
}
