use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_home() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "mux-version-metadata-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn version_does_not_bootstrap_or_rewrite_user_data() {
    let home = temporary_home();
    let mux_home = home.join(".mux");
    let settings = mux_home.join("settings.json");
    fs::create_dir_all(&mux_home).expect("create isolated MUX home");
    fs::write(&settings, b"{ intentionally invalid settings").expect("seed invalid settings");

    let output = Command::new(env!("CARGO_BIN_EXE_mux"))
        .arg("--version")
        .env("HOME", &home)
        .env("MUX_HOME", &mux_home)
        .output()
        .expect("run mux --version");

    assert!(
        output.status.success(),
        "mux --version failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("mux "));
    assert_eq!(
        fs::read(&settings).expect("read isolated settings"),
        b"{ intentionally invalid settings"
    );

    fs::remove_dir_all(home).expect("remove isolated MUX home");
}
