use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

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
    cmd.env("CODEX_HOME", &codex_dir)
        .arg("account")
        .arg("--format")
        .arg("json");

    cmd.assert()
        .success()
        .stdout(contains("tetel@05020324.xyz"))
        .stdout(contains("free"))
        .stdout(contains("account-123"));
}
