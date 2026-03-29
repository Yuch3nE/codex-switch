use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::model::{PrimaryRateLimit, TokenUsage, UsageSummary};

pub fn collect_usage(codex_home: &Path) -> anyhow::Result<UsageSummary> {
    let sessions_dir = codex_home.join("sessions");
    if !sessions_dir.exists() {
        return Ok(UsageSummary::empty());
    }

    let mut files = Vec::new();
    collect_rollout_files(&sessions_dir, &mut files)?;
    files.sort();

    let mut usage = UsageSummary::empty();
    usage.rollout_files = files.len();

    for file in files {
        if let Some(snapshot) = last_token_usage(&file)? {
            usage.rollout_files_with_token_count += 1;
            usage.aggregate_tokens.accumulate(&snapshot.usage);
            usage.latest_session_file = Some(file.display().to_string());
            usage.latest_session_tokens = Some(snapshot.usage);
            usage.primary = snapshot.primary;
            usage.secondary = snapshot.secondary;
            usage.plan_type = snapshot.plan_type;
        }
    }

    Ok(usage)
}

fn collect_rollout_files(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rollout_files(&path, files)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
        {
            files.push(path);
        }
    }

    Ok(())
}

fn last_token_usage(file: &Path) -> anyhow::Result<Option<TokenCountSnapshot>> {
    let contents = fs::read_to_string(file)?;
    let mut last_usage = None;

    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if value.get("type").and_then(Value::as_str) != Some("event_msg") {
            continue;
        }

        let payload = match value.get("payload") {
            Some(payload) => payload,
            None => continue,
        };

        if payload.get("type").and_then(Value::as_str) != Some("token_count") {
            continue;
        }

        let info = match payload.get("info") {
            Some(info) => info,
            None => continue,
        };

        let snapshot = info
            .get("total_token_usage")
            .or_else(|| info.get("last_token_usage"));

        if let Some(snapshot) = snapshot {
            let rate_limits = payload.get("rate_limits");
            last_usage = Some(TokenCountSnapshot {
                usage: parse_token_usage(snapshot),
                primary: rate_limits
                    .and_then(|value| value.get("primary"))
                    .map(parse_primary_rate_limit),
                secondary: rate_limits
                    .and_then(|value| value.get("secondary"))
                    .map(parse_primary_rate_limit),
                plan_type: rate_limits
                    .and_then(|value| value.get("plan_type"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            });
        }
    }

    Ok(last_usage)
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn parse_token_usage(value: &Value) -> TokenUsage {
    TokenUsage {
        input_tokens: u64_field(value, "input_tokens"),
        output_tokens: u64_field(value, "output_tokens"),
        reasoning_output_tokens: u64_field(value, "reasoning_output_tokens"),
        total_tokens: u64_field(value, "total_tokens"),
        cached_input_tokens: value.get("cached_input_tokens").and_then(Value::as_u64),
    }
}

fn parse_primary_rate_limit(value: &Value) -> PrimaryRateLimit {
    PrimaryRateLimit {
        used_percent: value
            .get("used_percent")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        window_minutes: u64_field(value, "window_minutes"),
        resets_at: u64_field(value, "resets_at"),
    }
}

struct TokenCountSnapshot {
    usage: TokenUsage,
    primary: Option<PrimaryRateLimit>,
    secondary: Option<PrimaryRateLimit>,
    plan_type: Option<String>,
}
