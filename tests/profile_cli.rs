use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use tempfile::tempdir;

fn rollout_with_primary(used_percent: f64, window_minutes: u64, resets_at: u64) -> String {
    format!(
        "{}\n",
        serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": 1,
                        "output_tokens": 2,
                        "reasoning_output_tokens": 3,
                        "total_tokens": 6
                    }
                },
                "rate_limits": {
                    "primary": {
                        "used_percent": used_percent,
                        "window_minutes": window_minutes,
                        "resets_at": resets_at
                    }
                }
            }
        })
    )
}

fn assert_auth_json_layout_matches_project_sample(contents: &str) {
    let auth_mode_index = contents.find("\"auth_mode\"").unwrap();
    let openai_api_key_index = contents.find("\"OPENAI_API_KEY\"").unwrap();
    let tokens_index = contents.find("\"tokens\"").unwrap();
    let last_refresh_index = contents.find("\"last_refresh\"").unwrap();

    assert!(auth_mode_index < openai_api_key_index);
    assert!(openai_api_key_index < tokens_index);
    assert!(tokens_index < last_refresh_index);

    let id_token_index = contents.find("\"id_token\"").unwrap();
    let access_token_index = contents.find("\"access_token\"").unwrap();
    let refresh_token_index = contents.find("\"refresh_token\"").unwrap();
    let account_id_index = contents.find("\"account_id\"").unwrap();

    assert!(id_token_index < access_token_index);
    assert!(access_token_index < refresh_token_index);
    assert!(refresh_token_index < account_id_index);
}

#[test]
fn profile_save_and_use_switches_auth_file() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let switch_dir = temp.path().join(".codex-auth-switch");
    fs::create_dir_all(&codex_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();
    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "alpha"])
        .assert()
        .success()
        .stdout(contains("已保存 profile: alpha"));

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth_alt.json"),
    )
    .unwrap();
    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "beta"])
        .assert()
        .success();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "use", "alpha"])
        .assert()
        .success()
        .stdout(contains("已切换到 profile: alpha"));

    let active_auth = fs::read_to_string(codex_dir.join("auth.json")).unwrap();
    let rollback_auth =
        fs::read_to_string(switch_dir.join("profiles/.rollback/auth.json")).unwrap();
    let state = fs::read_to_string(switch_dir.join("profiles/state.json")).unwrap();

    assert!(active_auth.contains("account-123"));
    assert!(rollback_auth.contains("account-456"));
    assert!(state.contains("alpha"));
}

#[test]
fn profile_list_reports_saved_profiles_and_active_one() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();
    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "alpha"])
        .assert()
        .success();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth_alt.json"),
    )
    .unwrap();
    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "beta"])
        .assert()
        .success();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "use", "beta"])
        .assert()
        .success();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "profile", "list"])
        .assert()
        .success()
        .stdout(contains("\"active_profile\": \"beta\""))
        .stdout(contains("\"name\": \"alpha\""))
        .stdout(contains("\"name\": \"beta\""))
        .stdout(contains("\"id\": \"alpha\""))
        .stdout(contains("\"email\": \"alt@example.com\""));
}

#[test]
fn profile_save_marks_current_auth_as_managed_active_profile() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "alpha"])
        .assert()
        .success();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "profile", "list"])
        .assert()
        .success()
        .stdout(contains("\"active_profile\": \"alpha\""))
        .stdout(contains("\"active\": true"));
}

#[test]
fn profile_save_without_name_uses_email_prefix_and_allows_duplicate_names() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save"])
        .assert()
        .success()
        .stdout(contains("已保存 profile: tetel (id: tetel)"));

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save"])
        .assert()
        .success()
        .stdout(contains("已保存 profile: tetel (id: tetel-2)"));

    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "profile", "list"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let profiles = value.get("profiles").and_then(Value::as_array).unwrap();

    assert_eq!(profiles.len(), 2);
    assert!(profiles
        .iter()
        .all(|profile| profile.get("name").and_then(Value::as_str) == Some("tetel")));
    assert_eq!(
        value.get("active_profile").and_then(Value::as_str),
        Some("tetel-2")
    );
}

#[test]
fn profile_use_rejects_ambiguous_duplicate_display_names() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save"])
        .assert()
        .success();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save"])
        .assert()
        .success();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "use", "tetel"])
        .assert()
        .failure()
        .stderr(contains("存在多个同名 profile，请使用 id 切换: tetel"));

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "use", "tetel-2"])
        .assert()
        .success()
        .stdout(contains("id: tetel-2"));
}

#[test]
fn profile_use_refreshes_previous_active_profile_snapshot() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let switch_dir = temp.path().join(".codex-auth-switch");
    let sessions_dir = codex_dir.join("sessions/2026/03/28");
    fs::create_dir_all(&sessions_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-alpha.jsonl"),
        rollout_with_primary(37.0, 10080, 1775210022),
    )
    .unwrap();
    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "alpha"])
        .assert()
        .success();

    fs::remove_dir_all(codex_dir.join("sessions")).unwrap();
    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth_alt.json"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-beta.jsonl"),
        rollout_with_primary(60.0, 43200, 1777812000),
    )
    .unwrap();
    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "beta"])
        .assert()
        .success();

    fs::remove_dir_all(codex_dir.join("sessions")).unwrap();
    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        sessions_dir.join("rollout-beta-new.jsonl"),
        rollout_with_primary(25.0, 43200, 1777812000),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "use", "alpha"])
        .assert()
        .success();

    let beta_profile = fs::read_to_string(switch_dir.join("profiles/beta/profile.json")).unwrap();
    let value: Value = serde_json::from_str(&beta_profile).unwrap();

    assert_eq!(
        value
            .get("primary")
            .and_then(|primary| primary.get("used_percent"))
            .and_then(Value::as_f64),
        Some(25.0)
    );
}

#[test]
fn profile_imports_single_auth_file_without_changing_active_profile() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let import_dir = temp.path().join("imports");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::create_dir_all(&import_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();
    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "alpha"])
        .assert()
        .success();

    let import_file = import_dir.join("auth.json");
    fs::write(&import_file, include_str!("fixtures/auth_alt.json")).unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "import", import_file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("已导入 1 个 profile"));

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "profile", "list"])
        .assert()
        .success()
        .stdout(contains("\"active_profile\": \"alpha\""))
        .stdout(contains("\"email\": \"alt@example.com\""));
}

#[test]
fn profile_import_recursively_scans_directory_for_auth_files() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let import_dir = temp.path().join("imports");
    let nested_dir = import_dir.join("nested/deeper");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::create_dir_all(&nested_dir).unwrap();

    fs::write(import_dir.join("auth.json"), include_str!("fixtures/auth.json")).unwrap();
    fs::write(
        nested_dir.join("auth.json"),
        include_str!("fixtures/auth_alt.json"),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "import", import_dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("已导入 2 个 profile"));

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "profile", "list"])
        .assert()
        .success()
        .stdout(contains("\"email\": \"tetel@05020324.xyz\""))
        .stdout(contains("\"email\": \"alt@example.com\""));
}

#[test]
fn profile_import_cpa_file_normalizes_into_standard_auth_json() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let switch_dir = temp.path().join(".codex-auth-switch");
    let import_dir = temp.path().join("imports");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::create_dir_all(&import_dir).unwrap();

    let import_file = import_dir.join("team.json");
    fs::write(&import_file, include_str!("fixtures/cpa_auth.json")).unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "import", "--cpa", import_file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("已导入 1 个 profile"));

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "profile", "list"])
        .assert()
        .success()
        .stdout(contains("\"email\": \"ohanna27@05020324.xyz\""))
        .stdout(contains("\"subscription_plan\": \"team\""));

    let normalized_auth = fs::read_to_string(switch_dir.join("profiles/ohanna27/auth.json")).unwrap();
    let value: Value = serde_json::from_str(&normalized_auth).unwrap();

    assert_eq!(value.get("auth_mode").and_then(Value::as_str), Some("chatgpt"));
    assert!(value.get("OPENAI_API_KEY").is_some());
    assert!(value.get("OPENAI_API_KEY").unwrap().is_null());
    assert_eq!(value.get("last_refresh").and_then(Value::as_str), Some("2026-03-28T00:28:17+08:00"));
    assert_eq!(
        value
            .get("tokens")
            .and_then(|tokens| tokens.get("account_id"))
            .and_then(Value::as_str),
        Some("account-team-789")
    );
    assert_eq!(
        value
            .get("tokens")
            .and_then(|tokens| tokens.get("refresh_token"))
            .and_then(Value::as_str),
        Some("rt_example")
    );
    assert!(value.get("refresh_token").is_none());
    assert!(value.get("access_token").is_none());
    assert_auth_json_layout_matches_project_sample(&normalized_auth);
}

#[test]
fn profile_import_auto_detects_cpa_file_without_flag() {
    let temp = tempdir().unwrap();
    let switch_dir = temp.path().join(".codex-auth-switch");
    let import_dir = temp.path().join("imports");
    fs::create_dir_all(&import_dir).unwrap();

    let import_file = import_dir.join("team.json");
    fs::write(&import_file, include_str!("fixtures/cpa_auth.json")).unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "import", import_file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("已导入 1 个 profile"));

    let normalized_auth = fs::read_to_string(switch_dir.join("profiles/ohanna27/auth.json")).unwrap();
    let value: Value = serde_json::from_str(&normalized_auth).unwrap();

    assert_eq!(value.get("auth_mode").and_then(Value::as_str), Some("chatgpt"));
    assert_eq!(
        value
            .get("tokens")
            .and_then(|tokens| tokens.get("account_id"))
            .and_then(Value::as_str),
        Some("account-team-789")
    );
}

#[test]
fn profile_use_writes_standard_auth_json_after_cpa_import() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let import_dir = temp.path().join("imports");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::create_dir_all(&import_dir).unwrap();

    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();

    let import_file = import_dir.join("team.json");
    fs::write(&import_file, include_str!("fixtures/cpa_auth.json")).unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "import", "--cpa", import_file.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "use", "ohanna27"])
        .assert()
        .success()
        .stdout(contains("已切换到 profile: ohanna27"));

    let current_auth = fs::read_to_string(codex_dir.join("auth.json")).unwrap();
    let value: Value = serde_json::from_str(&current_auth).unwrap();

    assert_eq!(value.get("auth_mode").and_then(Value::as_str), Some("chatgpt"));
    assert!(value.get("OPENAI_API_KEY").is_some());
    assert!(value.get("OPENAI_API_KEY").unwrap().is_null());
    assert!(value.get("tokens").is_some());
    assert_eq!(
        value
            .get("tokens")
            .and_then(|tokens| tokens.get("refresh_token"))
            .and_then(Value::as_str),
        Some("rt_example")
    );
    assert!(value.get("refresh_token").is_none());
    assert_auth_json_layout_matches_project_sample(&current_auth);
}

#[test]
fn profile_import_cpa_recursively_scans_json_files() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let import_dir = temp.path().join("imports");
    let nested_dir = import_dir.join("nested/deeper");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::create_dir_all(&nested_dir).unwrap();

    fs::write(import_dir.join("team-a.json"), include_str!("fixtures/cpa_auth.json")).unwrap();
    fs::write(nested_dir.join("team-b.json"), include_str!("fixtures/cpa_auth_alt.json")).unwrap();
    fs::write(import_dir.join("ignore.txt"), "not json").unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "import", "--cpa", import_dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("已导入 2 个 profile"));

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["--format", "json", "profile", "list"])
        .assert()
        .success()
        .stdout(contains("\"email\": \"ohanna27@05020324.xyz\""))
        .stdout(contains("\"email\": \"second@05020324.xyz\""));
}
