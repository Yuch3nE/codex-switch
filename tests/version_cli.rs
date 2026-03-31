use assert_cmd::Command;
use serde_json::Value;

#[test]
fn version_command_renders_text_output() {
    let version = env!("CARGO_PKG_VERSION");
    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .arg("version")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.contains("版本信息"));
    assert!(stdout.contains(version));
    assert!(stdout.contains("Git提交="));
    assert!(stdout.contains("Git引用="));
    assert!(stdout.contains("构建日期="));
    assert!(!stdout.contains('╭'));
    assert!(!stdout.contains('╰'));
    assert!(!stdout.contains('│'));
}

#[test]
fn version_command_renders_json_output() {
    let version = env!("CARGO_PKG_VERSION");
    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .args(["--format", "json", "version"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value.get("version").and_then(Value::as_str), Some(version));
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
