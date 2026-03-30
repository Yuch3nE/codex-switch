use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn doctor_text_reports_missing_files() {
    let temp = tempdir().unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("环境诊断"))
        .stdout(contains("auth.json"))
        .stdout(contains("缺失"));
}

#[test]
fn doctor_json_reports_basic_status() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let switch_dir = temp.path().join(".codex-auth-switch");

    fs::create_dir_all(switch_dir.join("profiles")).unwrap();
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(codex_dir.join("auth.json"), include_str!("fixtures/auth.json")).unwrap();
    fs::write(switch_dir.join("state.json"), "{\"active_profile\":\"alpha\"}").unwrap();
    fs::write(switch_dir.join("profiles/alpha.json"), "{\"name\":\"alpha\",\"auth\":{}}")
        .unwrap();

    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "doctor"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value.get("auth_exists").and_then(Value::as_bool), Some(true));
    assert_eq!(value.get("state_exists").and_then(Value::as_bool), Some(true));
    assert_eq!(value.get("profiles_count").and_then(Value::as_u64), Some(1));
    assert_eq!(
        value.get("active_profile").and_then(Value::as_str),
        Some("alpha")
    );
}
