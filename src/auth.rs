use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{jwt, model::AccountSummary};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthFile {
    pub auth_mode: String,
    #[serde(rename = "OPENAI_API_KEY", default = "null_json_value")]
    pub openai_api_key: Value,
    pub tokens: AuthTokens,
    #[serde(default, rename = "refresh_token", skip_serializing)]
    pub(crate) legacy_refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct AuthTokens {
    pub id_token: Option<String>,
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub account_id: Option<String>,
}

pub fn build_account_summary(codex_home: &Path) -> anyhow::Result<AccountSummary> {
    let auth_path = codex_home.join("auth.json");
    build_account_summary_from_path(&auth_path)
}

pub fn build_account_summary_from_path(path: &Path) -> anyhow::Result<AccountSummary> {
    let auth_file = load_auth_file(path)?;
    build_account_summary_from_auth_file(auth_file)
}

pub fn load_auth_file(path: &Path) -> anyhow::Result<AuthFile> {
    Ok(canonicalize_auth_file(serde_json::from_slice(&fs::read(path)?)?))
}

pub fn write_auth_file(path: &Path, auth_file: &AuthFile) -> anyhow::Result<()> {
    let auth_file = canonicalize_auth_file(auth_file.clone());
    fs::write(path, serde_json::to_vec_pretty(&auth_file)?)?;
    Ok(())
}

pub fn build_account_summary_from_auth_file(auth_file: AuthFile) -> anyhow::Result<AccountSummary> {
    let id_payload = auth_file
        .tokens
        .id_token
        .as_deref()
        .map(jwt::decode_payload)
        .transpose()?;
    let access_payload = auth_file
        .tokens
        .access_token
        .as_deref()
        .map(jwt::decode_payload)
        .transpose()?;

    let auth_meta = id_payload
        .as_ref()
        .and_then(|value| value.get("https://api.openai.com/auth"))
        .or_else(|| {
            access_payload
                .as_ref()
                .and_then(|value| value.get("https://api.openai.com/auth"))
        });
    let profile_meta = access_payload
        .as_ref()
        .and_then(|value| value.get("https://api.openai.com/profile"));

    Ok(AccountSummary {
        auth_mode: auth_file.auth_mode,
        account_id: auth_file.tokens.account_id,
        user_id: extract_string(auth_meta, "user_id"),
        email: extract_root_string(id_payload.as_ref(), "email")
            .or_else(|| extract_string(profile_meta, "email")),
        email_verified: extract_root_bool(id_payload.as_ref(), "email_verified")
            .or_else(|| extract_bool(profile_meta, "email_verified")),
        name: extract_root_string(id_payload.as_ref(), "name"),
        subscription_plan: extract_string(auth_meta, "chatgpt_plan_type"),
        last_refresh: auth_file.last_refresh,
        organization_count: auth_meta
            .and_then(|value| value.get("organizations"))
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0),
    })
}

pub fn canonicalize_auth_file(mut auth_file: AuthFile) -> AuthFile {
    if auth_file.tokens.refresh_token.is_none() {
        auth_file.tokens.refresh_token = auth_file.legacy_refresh_token.take();
    }

    auth_file
}

fn null_json_value() -> Value {
    Value::Null
}

fn extract_string(value: Option<&Value>, key: &str) -> Option<String> {
    value?.get(key)?.as_str().map(ToOwned::to_owned)
}

fn extract_bool(value: Option<&Value>, key: &str) -> Option<bool> {
    value?.get(key)?.as_bool()
}

fn extract_root_string(value: Option<&Value>, key: &str) -> Option<String> {
    value?.get(key)?.as_str().map(ToOwned::to_owned)
}

fn extract_root_bool(value: Option<&Value>, key: &str) -> Option<bool> {
    value?.get(key)?.as_bool()
}
