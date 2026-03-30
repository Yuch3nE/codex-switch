use std::fs;

use assert_cmd::Command;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_match};
use serde_json::Value;
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

fn rollout_with_limits(
    plan_type: &str,
    primary: Option<(f64, u64, u64)>,
    secondary: Option<(f64, u64, u64)>,
) -> String {
    let mut rate_limits = serde_json::json!({
        "limit_id": "codex",
        "limit_name": null,
        "credits": null,
        "plan_type": plan_type,
    });

    if let Some((used_percent, window_minutes, resets_at)) = primary {
        rate_limits["primary"] = serde_json::json!({
            "used_percent": used_percent,
            "window_minutes": window_minutes,
            "resets_at": resets_at,
        });
    }

    if let Some((used_percent, window_minutes, resets_at)) = secondary {
        rate_limits["secondary"] = serde_json::json!({
            "used_percent": used_percent,
            "window_minutes": window_minutes,
            "resets_at": resets_at,
        });
    }

    format!(
        "{}\n",
        serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": 50362,
                        "cached_input_tokens": 27008,
                        "output_tokens": 476,
                        "reasoning_output_tokens": 275,
                        "total_tokens": 50838
                    },
                    "last_token_usage": {
                        "input_tokens": 26010,
                        "cached_input_tokens": 24576,
                        "output_tokens": 152,
                        "reasoning_output_tokens": 56,
                        "total_tokens": 26162
                    },
                    "model_context_window": 258400
                },
                "rate_limits": rate_limits
            }
        })
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
    fn completions_generates_script_for_powershell() {
        let mut cmd = Command::cargo_bin("codex-switch").unwrap();
        cmd.args(["completions", "powershell"]);

        cmd.assert()
        .success()
        .stdout(contains("codex-switch"));
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
        sessions_dir.join("rollout-b.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":5,\"output_tokens\":7,\"reasoning_output_tokens\":11,\"total_tokens\":23}},\"rate_limits\":{\"primary\":{\"used_percent\":60.0,\"window_minutes\":43200,\"resets_at\":1777812000}}}}\n",
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "beta"])
        .assert()
        .success();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("HOME", temp.path())
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
        sessions_dir.join("rollout-b.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":5,\"output_tokens\":7,\"reasoning_output_tokens\":11,\"total_tokens\":23}},\"rate_limits\":{\"primary\":{\"used_percent\":60.0,\"window_minutes\":43200,\"resets_at\":1777812000}}}}\n",
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
        .env("HOME", temp.path())
        .args(["profile", "save", "gamma"])
        .assert()
        .success();

    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
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
    assert!(stdout.contains("5H额度"));
    assert!(stdout.contains("周重置"));
    assert!(stdout.contains("2026-05-03 20:40:00"));
    assert!(!stdout.contains("CST"));
    assert!(stdout.contains("╞"));
    assert!(stdout.contains("╘"));
    assert!(stdout.lines().filter(|line| line.contains('┼')).count() >= 3);

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
    let switch_dir = temp.path().join(".codex-auth-switch");
    let profiles_dir = switch_dir.join("profiles");
    let sessions_dir = codex_dir.join("sessions/2026/03/28");

    fs::create_dir_all(&profiles_dir).unwrap();
    fs::create_dir_all(&sessions_dir).unwrap();

    let auth_value: Value = serde_json::from_str(include_str!("fixtures/auth.json")).unwrap();
    fs::write(
        profiles_dir.join("alpha.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "alpha",
            "email": "tetel@05020324.xyz",
            "auth": auth_value
        })).unwrap(),
    ).unwrap();
    fs::write(
        switch_dir.join("state.json"),
        "{\"active_profile\":\"alpha\"}",
    ).unwrap();
    fs::write(
        sessions_dir.join("rollout-active.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":10,\"output_tokens\":20,\"reasoning_output_tokens\":30,\"total_tokens\":60}},\"rate_limits\":{\"primary\":{\"used_percent\":37.0,\"window_minutes\":10080,\"resets_at\":1775210022}}}}\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("HOME", temp.path()).arg("usage");

    cmd.assert()
        .success()
        .stdout(contains("● tetel@05020324.xyz"))
        .stdout(contains("╢████████████░░░░░░░░╟ 63.0%"))
        .stdout(contains("2026-04-03 17:53:42"))
        .stdout(is_match("CST").unwrap().not());
}

#[test]
fn usage_prefers_active_profile_realtime_snapshot_over_saved_snapshot() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let switch_dir = temp.path().join(".codex-auth-switch");
    let profiles_dir = switch_dir.join("profiles");
    let sessions_dir = codex_dir.join("sessions/2026/03/28");

    fs::create_dir_all(&profiles_dir).unwrap();
    fs::create_dir_all(&sessions_dir).unwrap();

    let auth_value: Value = serde_json::from_str(include_str!("fixtures/auth.json")).unwrap();
    fs::write(
        profiles_dir.join("alpha.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "name": "alpha",
            "email": "tetel@05020324.xyz",
            "primary": {"used_percent": 60.0, "window_minutes": 43200, "resets_at": 1777812000},
            "auth": auth_value
        })).unwrap(),
    ).unwrap();
    fs::write(
        switch_dir.join("state.json"),
        "{\"active_profile\":\"alpha\"}",
    ).unwrap();
    fs::write(
        sessions_dir.join("rollout-active.jsonl"),
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":10,\"output_tokens\":20,\"reasoning_output_tokens\":30,\"total_tokens\":60}},\"rate_limits\":{\"primary\":{\"used_percent\":37.0,\"window_minutes\":10080,\"resets_at\":1775210022}}}}\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codex-switch").unwrap();
    cmd.env("HOME", temp.path()).arg("usage");

    cmd.assert()
        .success()
        .stdout(contains("╢████████████░░░░░░░░╟ 63.0%"))
        .stdout(contains("2026-04-03 17:53:42"))
        .stdout(contains("● tetel@05020324.xyz"))
        .stdout(contains("周重置"))
        .stdout(is_match("CST").unwrap().not())
        .stdout(is_match("40.0%").unwrap().not());
}

#[test]
fn usage_json_exposes_primary_secondary_and_plan_type() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let sessions_dir = codex_dir.join("sessions/2026/03/29");

    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        auth_fixture_with_email("team@example.com", "account-team"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-team.jsonl"),
        rollout_with_limits(
            "team",
            Some((0.0, 300, 1774794986)),
            Some((3.0, 10080, 1775190114)),
        ),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "team"])
        .assert()
        .success();

    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["usage", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let profiles = value.get("profiles").and_then(Value::as_array).unwrap();
    let team = profiles
        .iter()
        .find(|profile| profile.get("email").and_then(Value::as_str) == Some("team@example.com"))
        .unwrap();

    assert_eq!(team.get("plan_type").and_then(Value::as_str), Some("team"));
    assert_eq!(
        team
            .get("primary")
            .and_then(|limit| limit.get("window_minutes"))
            .and_then(Value::as_u64),
        Some(300)
    );
    assert_eq!(
        team
            .get("secondary")
            .and_then(|limit| limit.get("window_minutes"))
            .and_then(Value::as_u64),
        Some(10080)
    );
}

#[test]
fn usage_text_renders_team_dual_limits_and_free_weekly_limit() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let sessions_dir = codex_dir.join("sessions/2026/03/29");

    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        auth_fixture_with_email("team@example.com", "account-team"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-team.jsonl"),
        rollout_with_limits(
            "team",
            Some((0.0, 300, 1774794986)),
            Some((3.0, 10080, 1775190114)),
        ),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "team"])
        .assert()
        .success();

    fs::remove_dir_all(codex_dir.join("sessions")).unwrap();
    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        auth_fixture_with_email("free@example.com", "account-free"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-free.jsonl"),
        rollout_with_limits("free", Some((40.0, 10080, 1775190114)), None),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "free"])
        .assert()
        .success();

    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .arg("usage")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("5H额度"));
    assert!(stdout.contains("5H重置"));
    assert!(stdout.contains("周额度"));
    assert!(stdout.contains("周重置"));
    assert!(stdout.contains("team@example.com"));
    assert!(stdout.contains("free@example.com"));
    assert!(stdout.contains("╢████████████████████╟ 100.0%"));
    assert!(stdout.contains("╢███████████████████░╟ 97.0%"));
    assert!(stdout.contains("╢████████████░░░░░░░░╟ 60.0%"));
    assert!(stdout.contains("未知"));
}

#[test]
fn usage_text_marks_low_remaining_with_warning_symbol() {
    let temp = tempdir().unwrap();
    let codex_dir = temp.path().join(".codex");
    let sessions_dir = codex_dir.join("sessions/2026/03/29");

    fs::create_dir_all(&sessions_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        auth_fixture_with_email("warn@example.com", "account-warn"),
    )
    .unwrap();
    fs::write(
        sessions_dir.join("rollout-warn.jsonl"),
        rollout_with_limits("plus", Some((85.0, 300, 1774794986)), None),
    )
    .unwrap();

    Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .args(["profile", "save", "warn"])
        .assert()
        .success();

    let output = Command::cargo_bin("codex-switch")
        .unwrap()
        .env("HOME", temp.path())
        .arg("usage")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("15.0% ⚠"));
}
