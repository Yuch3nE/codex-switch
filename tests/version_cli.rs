use assert_cmd::Command;
use predicates::str::{contains, is_match};
use serde_json::Value;

#[test]
fn version_command_renders_text_output() {
    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.arg("version");

    cmd.assert()
        .success()
        .stdout(contains("版本信息"))
        .stdout(contains("0.1.0"))
        .stdout(is_match(r"Git 提交\s*│\s*[0-9a-f]{7,}").unwrap())
        .stdout(is_match(r"构建日期\s*│\s*\d{4}-\d{2}-\d{2}T").unwrap());
}

#[test]
fn version_command_renders_json_output() {
    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .args(["--format", "json", "version"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value.get("version").and_then(Value::as_str), Some("0.1.0"));
    assert!(value
        .get("git_commit")
        .and_then(Value::as_str)
        .map(|sha| sha.len() >= 7)
        .unwrap_or(false));
    assert!(value
        .get("git_ref")
        .and_then(Value::as_str)
        .map(|git_ref| !git_ref.is_empty())
        .unwrap_or(false));
    assert!(value
        .get("build_date")
        .and_then(Value::as_str)
        .map(|date| date.contains('T'))
        .unwrap_or(false));
}
