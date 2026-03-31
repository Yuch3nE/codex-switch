use assert_cmd::Command;
use predicates::str::{contains, is_match};

#[test]
fn version_command_renders_text_output() {
    let version = env!("CARGO_PKG_VERSION");
    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.arg("version");

    cmd.assert()
        .success()
        .stdout(contains("版本信息"))
        .stdout(contains(version))
        .stdout(is_match(r"Git 提交\s*│\s*[0-9a-f]{7,}").unwrap())
        .stdout(is_match(r"构建日期\s*│\s*\d{4}-\d{2}-\d{2}T").unwrap());
}

#[test]
fn version_command_ignores_json_flag() {
    let version = env!("CARGO_PKG_VERSION");
    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .args(["--format", "json", "version"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("版本信息"));
    assert!(stdout.contains(version));
    assert!(serde_json::from_str::<serde_json::Value>(&stdout).is_err());
}
