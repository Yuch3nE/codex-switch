use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use tempfile::tempdir;

fn assert_json_matches_golden(actual: &[u8]) {
    let actual_value: Value = serde_json::from_slice(actual).unwrap();
    let golden_value: Value =
        serde_json::from_str(include_str!("fixtures/golden/account.json")).unwrap();
    assert_eq!(actual_value, golden_value);
}

#[test]
fn account_command_reads_email_and_plan_from_auth_file() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("HOME", temp.path())
        .arg("account")
        .arg("--format")
        .arg("json");

    cmd.assert()
        .success()
        .stdout(contains("tetel@05020324.xyz"))
        .stdout(contains("free"))
        .stdout(contains("account-123"));
}

#[test]
fn account_command_accepts_auth_without_auth_mode_field() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    let mut auth_value: Value = serde_json::from_str(include_str!("fixtures/auth.json")).unwrap();
    auth_value.as_object_mut().unwrap().remove("auth_mode");
    fs::write(
        codex_dir.join("auth.json"),
        serde_json::to_string_pretty(&auth_value).unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("HOME", temp.path())
        .arg("account")
        .arg("--format")
        .arg("json");

    cmd.assert()
        .success()
        .stdout(contains("tetel@05020324.xyz"))
        .stdout(contains("\"auth_mode\": \"chatgpt\""));
}

#[test]
fn account_command_accepts_auth_without_tokens_field() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        "{\n  \"OPENAI_API_KEY\": \"sk-test\"\n}",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("HOME", temp.path())
        .arg("account")
        .arg("--format")
        .arg("json");

    cmd.assert()
        .success()
        .stdout(contains("\"auth_mode\": \"chatgpt\""))
        .stdout(contains("\"account_id\": null"));
}

#[test]
fn account_json_matches_golden_contract() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();

    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "account"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_json_matches_golden(&output.stdout);
}
