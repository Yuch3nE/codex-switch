use anyhow::{anyhow, Context};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::Value;

pub fn decode_payload(token: &str) -> anyhow::Result<Value> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow!("missing jwt payload"))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .with_context(|| "failed to decode jwt payload")?;
    Ok(serde_json::from_slice(&decoded).with_context(|| "failed to parse jwt payload json")?)
}
