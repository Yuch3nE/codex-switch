use std::fs;

use assert_cmd::Command;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_match};
use tempfile::tempdir;

fn auth_fixture_with_email(email: &str, account_id: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let id_payload = URL_SAFE_NO_PAD.encode(format!(
        "{{\"email\":\"{email}\",\"email_verified\":true,\"name\":\"Gamma\",\"https://api.openai.com/auth\":{{\"chatgpt_plan_type\":\"free\",\"user_id\":\"user-789\",\"organizations\":[{{\"id\":\"org-3\"}}]}}}}"
    ));
    let access_payload = URL_SAFE_NO_PAD.encode(format!(
        "{{\"https://api.openai.com/profile\":{{\"email\":\"{email}\",\"email_verified\":true}},\"https://api.openai.com/auth\":{{\"chatgpt_plan_type\":\"free\",\"user_id\":\"user-789\"}}}}"
    ));

    format!(
        "{{\n  \"auth_mode\": \"chatgpt\",\n  \"tokens\": {{\n    \"id_token\": \"{header}.{id_payload}.sig\",\n    \"access_token\": \"{header}.{access_payload}.sig\",\n    \"account_id\": \"{account_id}\"\n  }},\n  \"last_refresh\": \"2026-03-29T08:00:00+08:00\"\n}}"
    )
}

#[test]
fn help_lists_usage_and_hides_summary() {
    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(contains("account"))
        .stdout(contains("usage"))
        .stdout(contains("profile"))
        .stdout(is_match(r"(?m)^\s*summary\s*$").unwrap().not());
}

#[test]
fn usage_command_merges_account_and_usage_in_json() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let sessions_dir = codex_dir.join("sessions/2026/03/28");

    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-a.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":10,\"output_tokens\":20,\"reasoning_output_tokens\":30,\"total_tokens\":60}},\"rate_limits\":{\"primary\":{\"used_percent\":37.0,\"window_minutes\":10080,\"resets_at\":1775210022}}}}\n",
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("CODEX_HOME", &codex_dir)
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
        sessions_dir.join("rollout-b.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":5,\"output_tokens\":7,\"reasoning_output_tokens\":11,\"total_tokens\":23}},\"rate_limits\":{\"primary\":{\"used_percent\":60.0,\"window_minutes\":43200,\"resets_at\":1777812000}}}}\n",
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("CODEX_HOME", &codex_dir)
        .args(["profile", "save", "beta"])
        .assert()
        .success();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("CODEX_HOME", &codex_dir)
        .arg("usage")
        .arg("--format")
        .arg("json");

    cmd.assert()
        .success()
        .stdout(contains("\"profiles\""))
        .stdout(contains("tetel@05020324.xyz"))
        .stdout(contains("alt@example.com"))
        .stdout(contains("\"used_percent\": 37.0"))
        .stdout(contains("\"used_percent\": 60.0"))
        .stdout(contains("\"window_minutes\": 43200"))
        .stdout(contains("\"resets_at\": 1777812000"));
}

#[test]
fn usage_text_renders_single_account_quota_table() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let sessions_dir = codex_dir.join("sessions/2026/03/28");

    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-a.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":10,\"output_tokens\":20,\"reasoning_output_tokens\":30,\"total_tokens\":60}},\"rate_limits\":{\"primary\":{\"used_percent\":37.0,\"window_minutes\":10080,\"resets_at\":1775210022}}}}\n",
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("CODEX_HOME", &codex_dir)
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
        sessions_dir.join("rollout-b.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":5,\"output_tokens\":7,\"reasoning_output_tokens\":11,\"total_tokens\":23}},\"rate_limits\":{\"primary\":{\"used_percent\":60.0,\"window_minutes\":43200,\"resets_at\":1777812000}}}}\n",
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("CODEX_HOME", &codex_dir)
        .args(["profile", "save", "beta"])
        .assert()
        .success();

    fs::remove_dir_all(codex_dir.join("sessions")).unwrap();
    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        auth_fixture_with_email("gamma@example.com", "account-789"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-c.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":8,\"output_tokens\":3,\"reasoning_output_tokens\":1,\"total_tokens\":12}},\"rate_limits\":{\"primary\":{\"used_percent\":10.0,\"window_minutes\":10080,\"resets_at\":1775210022}}}}\n",
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("CODEX_HOME", &codex_dir)
        .args(["profile", "save", "gamma"])
        .assert()
        .success();

    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .env("CODEX_HOME", &codex_dir)
        .arg("usage")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("账号额度总览"));
    assert!(stdout.contains("PLUS"));
    assert!(stdout.contains("FREE"));
    assert!(stdout.contains("alt@example.com"));
    assert!(stdout.contains("● gamma@example.com"));
    assert!(stdout.contains("tetel@05020324.xyz"));
    assert!(stdout.contains("╢████████░░░░░░░░░░░░╟ 40.0%"));
    assert!(stdout.contains("╢██████████████████░░╟ 90.0%"));
    assert!(stdout.contains("╢████████████░░░░░░░░╟ 63.0%"));
    assert!(stdout.contains("30 天"));

    let plus_index = stdout.find("PLUS").unwrap();
    let free_index = stdout.find("FREE").unwrap();
    let gamma_index = stdout.find("gamma@example.com").unwrap();
    let tetel_index = stdout.find("tetel@05020324.xyz").unwrap();

    assert!(plus_index < free_index);
    assert!(gamma_index < tetel_index);
}

#[test]
fn usage_falls_back_to_active_profile_current_session_snapshot() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let profile_dir = codex_dir.join("profiles/alpha");
    let sessions_dir = codex_dir.join("sessions/2026/03/28");

    fs::create_dir_all(&profile_dir).unwrap();
    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        profile_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();
    fs::write(profile_dir.join("profile.json"), "{\"name\":\"alpha\"}").unwrap();
    fs::write(
        codex_dir.join("profiles/state.json"),
        "{\"active_profile\":\"alpha\"}",
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-active.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":10,\"output_tokens\":20,\"reasoning_output_tokens\":30,\"total_tokens\":60}},\"rate_limits\":{\"primary\":{\"used_percent\":37.0,\"window_minutes\":10080,\"resets_at\":1775210022}}}}\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("CODEX_HOME", &codex_dir).arg("usage");

    cmd.assert()
        .success()
        .stdout(contains("● tetel@05020324.xyz"))
        .stdout(contains("╢████████████░░░░░░░░╟ 63.0%"))
        .stdout(contains("2026-04-03 17:53:42 CST"));
}

#[test]
fn usage_prefers_active_profile_realtime_snapshot_over_saved_snapshot() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let profile_dir = codex_dir.join("profiles/alpha");
    let sessions_dir = codex_dir.join("sessions/2026/03/28");

    fs::create_dir_all(&profile_dir).unwrap();
    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        profile_dir.join("auth.json"),
        include_str!("fixtures/auth.json"),
    )
    .unwrap();
    fs::write(
        profile_dir.join("profile.json"),
        "{\"name\":\"alpha\",\"primary\":{\"used_percent\":60.0,\"window_minutes\":43200,\"resets_at\":1777812000}}",
    )
    .unwrap();
    fs::write(
        codex_dir.join("profiles/state.json"),
        "{\"active_profile\":\"alpha\"}",
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-active.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":10,\"output_tokens\":20,\"reasoning_output_tokens\":30,\"total_tokens\":60}},\"rate_limits\":{\"primary\":{\"used_percent\":37.0,\"window_minutes\":10080,\"resets_at\":1775210022}}}}\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("CODEX_HOME", &codex_dir).arg("usage");

    cmd.assert()
        .success()
        .stdout(contains("╢████████████░░░░░░░░╟ 63.0%"))
        .stdout(contains("7 天"))
        .stdout(contains("2026-04-03 17:53:42 CST"))
        .stdout(contains("● tetel@05020324.xyz"))
        .stdout(is_match("40.0%").unwrap().not());
}
