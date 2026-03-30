use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    auth,
    model::{PrimaryRateLimit, ProfileListOutput, ProfileSummary},
    sessions,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportFormat {
    Standard,
    Cpa,
}

pub fn save_profile(
    codex_home: &Path,
    switch_home: &Path,
    name: Option<&str>,
) -> anyhow::Result<String> {
    let current_auth = codex_home.join("auth.json");
    if !current_auth.exists() {
        bail!("当前 auth.json 不存在");
    }

    let auth_file = auth::load_auth_file(&current_auth)?;
    let summary = auth::build_account_summary_from_auth_file(auth_file.clone())?;
    let display_name = resolve_display_name(name, summary.email.as_deref())?;
    let profile_id = generate_profile_id(switch_home, &display_name)?;
    let usage = current_usage_snapshot(codex_home)?;

    write_flat_profile(
        switch_home,
        &profile_id,
        &FlatProfile {
            name: display_name.clone(),
            email: summary.email,
            subscription_plan: summary.subscription_plan,
            account_id: summary.account_id,
            plan_type: usage.plan_type,
            primary: usage.primary,
            secondary: usage.secondary,
            auth: serde_json::to_value(auth_file)?,
        },
    )?;
    write_state(
        switch_home,
        &ProfilesState {
            active_profile: Some(profile_id.clone()),
        },
    )?;

    Ok(format!(
        "已保存 profile: {} (id: {})",
        display_name, profile_id
    ))
}

pub fn import_profiles(
    _codex_home: &Path,
    switch_home: &Path,
    path: &Path,
    format: ImportFormat,
) -> anyhow::Result<String> {
    let state = read_state(switch_home)?;
    let files = collect_import_files(path, format)?;

    if files.is_empty() {
        match format {
            ImportFormat::Standard => bail!("未找到可导入的 auth.json"),
            ImportFormat::Cpa => bail!("未找到可导入的 CPA .json 文件"),
        }
    }

    for file in &files {
        import_profile_from_path(switch_home, file, format)?;
    }

    write_state(switch_home, &state)?;

    Ok(format!("已导入 {} 个 profile", files.len()))
}

pub fn use_profile(codex_home: &Path, switch_home: &Path, name: &str) -> anyhow::Result<String> {
    refresh_active_profile_snapshot(codex_home, switch_home)?;

    let resolved = resolve_profile(switch_home, name)?;

    let current_auth = codex_home.join("auth.json");
    let rollback_dir = profiles_root(switch_home).join(".rollback");
    fs::create_dir_all(&rollback_dir)?;

    if current_auth.exists() {
        copy_auth_json_with_canonicalization(&current_auth, &rollback_dir.join("auth.json"))?;
    }

    auth::write_auth_file(&current_auth, &resolved.auth)?;
    write_state(
        switch_home,
        &ProfilesState {
            active_profile: Some(resolved.id.clone()),
        },
    )?;

    Ok(format!(
        "已切换到 profile: {} (id: {})",
        resolved.name, resolved.id
    ))
}

pub fn delete_profiles(switch_home: &Path, profile_ids: &[&str]) -> anyhow::Result<String> {
    if profile_ids.is_empty() {
        bail!("未选择要删除的 profile");
    }

    let state = read_state(switch_home)?;
    if let Some(active_profile) = state.active_profile.as_deref() {
        if profile_ids.iter().any(|profile_id| *profile_id == active_profile) {
            bail!("当前激活的 profile 不允许删除，请先切换到其他 profile");
        }
    }

    let mut deleted_ids = Vec::new();
    for profile_id in profile_ids {
        let resolved = resolve_profile(switch_home, profile_id)?;
        fs::remove_file(flat_profile_path(switch_home, &resolved.id))?;
        deleted_ids.push(resolved.id);
    }

    Ok(format!(
        "已删除 {} 个 profile: {}",
        deleted_ids.len(),
        deleted_ids.join(", ")
    ))
}

pub fn list_profiles(codex_home: &Path, switch_home: &Path) -> anyhow::Result<ProfileListOutput> {
    let root = profiles_root(switch_home);
    fs::create_dir_all(&root)?;
    let state = read_state(switch_home)?;
    let active_usage = current_usage_snapshot(codex_home)?;

    // 第一轮：惰性迁移旧格式目录
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if id.starts_with('.') || !path.join("auth.json").exists() {
            continue;
        }
        migrate_legacy_profile(switch_home, &path, id)?;
    }

    // 第二轮：读取扁平 .json 文件
    let mut profiles = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if file_name == "state.json" || file_name.starts_with('.') {
            continue;
        }
        let Some(id) = file_name.strip_suffix(".json") else {
            continue;
        };
        let flat: FlatProfile = serde_json::from_slice(&fs::read(&path)?)
            .with_context(|| format!("failed to read profile {id}"))?;
        let is_active = state.active_profile.as_deref() == Some(id);

        profiles.push(ProfileSummary {
            active: is_active,
            id: id.to_string(),
            name: flat.name,
            email: flat.email,
            subscription_plan: flat.subscription_plan,
            account_id: flat.account_id,
            primary: if is_active {
                active_usage.primary.clone().or(flat.primary)
            } else {
                flat.primary
            },
            secondary: if is_active {
                active_usage.secondary.clone().or(flat.secondary)
            } else {
                flat.secondary
            },
            plan_type: if is_active {
                active_usage.plan_type.clone().or(flat.plan_type)
            } else {
                flat.plan_type
            },
        });
    }

    profiles.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));

    Ok(ProfileListOutput {
        active_profile: state.active_profile,
        profiles,
    })
}

fn profiles_root(codex_home: &Path) -> PathBuf {
    codex_home.join("profiles")
}

fn collect_files_with_predicate<F>(dir: &Path, files: &mut Vec<PathBuf>, pred: &F) -> anyhow::Result<()>
where
    F: Fn(&Path) -> bool,
{
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_predicate(&path, files, pred)?;
        } else if pred(&path) {
            files.push(path);
        }
    }
    files.sort();
    Ok(())
}

fn collect_auth_files(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    collect_files_with_predicate(dir, files, &|path| {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "auth.json")
    })
}

fn collect_cpa_files(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    collect_files_with_predicate(dir, files, &|path| {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    })
}

fn collect_import_files(path: &Path, format: ImportFormat) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.to_path_buf());
    } else if path.is_dir() {
        match format {
            ImportFormat::Standard => collect_auth_files(path, &mut files)?,
            ImportFormat::Cpa => collect_cpa_files(path, &mut files)?,
        }
    } else {
        bail!("导入路径不存在: {}", path.display());
    }

    Ok(files)
}

fn import_profile_from_path(
    switch_home: &Path,
    path: &Path,
    format: ImportFormat,
) -> anyhow::Result<()> {
    let auth_file = load_import_auth_file(path, format)?;
    let summary = auth::build_account_summary_from_auth_file(auth_file.clone())?;
    let display_name = resolve_display_name(None, summary.email.as_deref())?;
    let profile_id = generate_profile_id(switch_home, &display_name)?;

    write_flat_profile(
        switch_home,
        &profile_id,
        &FlatProfile {
            name: display_name,
            email: summary.email,
            subscription_plan: summary.subscription_plan,
            account_id: summary.account_id,
            plan_type: None,
            primary: None,
            secondary: None,
            auth: serde_json::to_value(auth_file)?,
        },
    )
}

fn load_import_auth_file(path: &Path, format: ImportFormat) -> anyhow::Result<auth::AuthFile> {
    match format {
        ImportFormat::Standard => load_standard_or_cpa_auth_file(path),
        ImportFormat::Cpa => load_cpa_auth_file(path),
    }
}

fn load_standard_or_cpa_auth_file(path: &Path) -> anyhow::Result<auth::AuthFile> {
    let contents = fs::read(path)?;
    let value: Value = serde_json::from_slice(&contents)?;

    if looks_like_standard_auth(&value) {
        return Ok(serde_json::from_value(value)?);
    }

    if looks_like_cpa_auth(&value) {
        let cpa: CpaAuthFile = serde_json::from_value(value)?;
        return cpa_auth_to_auth_file(cpa, path);
    }

    bail!("不支持的鉴权文件格式: {}", path.display())
}

fn looks_like_standard_auth(value: &Value) -> bool {
    value
        .get("auth_mode")
        .and_then(Value::as_str)
        .is_some()
        && value.get("tokens").and_then(Value::as_object).is_some()
}

fn looks_like_cpa_auth(value: &Value) -> bool {
    value.get("access_token").and_then(Value::as_str).is_some()
        && value.get("id_token").and_then(Value::as_str).is_some()
        && value.get("account_id").and_then(Value::as_str).is_some()
}

#[derive(Debug, Deserialize)]
struct CpaAuthFile {
    access_token: String,
    id_token: String,
    account_id: String,
    refresh_token: Option<String>,
    last_refresh: Option<String>,
}

fn load_cpa_auth_file(path: &Path) -> anyhow::Result<auth::AuthFile> {
    let cpa: CpaAuthFile = serde_json::from_slice(&fs::read(path)?)
        .with_context(|| format!("failed to parse CPA auth file: {}", path.display()))?;

    cpa_auth_to_auth_file(cpa, path)
}

fn cpa_auth_to_auth_file(cpa: CpaAuthFile, path: &Path) -> anyhow::Result<auth::AuthFile> {
    if cpa.access_token.trim().is_empty() {
        bail!("CPA 鉴权文件缺少 access_token: {}", path.display());
    }
    if cpa.id_token.trim().is_empty() {
        bail!("CPA 鉴权文件缺少 id_token: {}", path.display());
    }
    if cpa.account_id.trim().is_empty() {
        bail!("CPA 鉴权文件缺少 account_id: {}", path.display());
    }

    Ok(auth::AuthFile {
        auth_mode: "chatgpt".to_string(),
        openai_api_key: Value::Null,
        tokens: auth::AuthTokens {
            id_token: Some(cpa.id_token),
            access_token: Some(cpa.access_token),
            refresh_token: cpa.refresh_token,
            account_id: Some(cpa.account_id),
        },
        legacy_refresh_token: None,
        last_refresh: cpa.last_refresh,
    })
}

fn copy_auth_json_with_canonicalization(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let auth_file = auth::load_auth_file(source)?;
    auth::write_auth_file(destination, &auth_file)
}

fn state_path(switch_home: &Path) -> PathBuf {
    profiles_root(switch_home).join("state.json")
}

fn validate_profile_name(name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        return Err(anyhow!("profile 名称不能为空"));
    }

    Ok(())
}

fn resolve_display_name(name: Option<&str>, email: Option<&str>) -> anyhow::Result<String> {
    let candidate = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            email
                .and_then(|value| value.split('@').next())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "profile".to_string());

    validate_profile_name(&candidate)?;
    Ok(candidate)
}

fn generate_profile_id(switch_home: &Path, display_name: &str) -> anyhow::Result<String> {
    let root = profiles_root(switch_home);
    fs::create_dir_all(&root)?;

    let base = slugify_profile_id(display_name);
    let mut candidate = base.clone();
    let mut suffix = 2usize;

    // 同时检查新格式 (.json) 和旧格式 (目录) 以避免迁移期间冲突
    while root.join(format!("{}.json", candidate)).exists() || root.join(&candidate).is_dir() {
        candidate = format!("{}-{}", base, suffix);
        suffix += 1;
    }

    Ok(candidate)
}

fn slugify_profile_id(display_name: &str) -> String {
    let mut slug = String::new();

    for ch in display_name.trim().chars() {
        if ch.is_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if matches!(ch, ' ' | '-' | '_') && !slug.ends_with('-') {
            slug.push('-');
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "profile".to_string()
    } else {
        slug
    }
}

fn profile_metadata_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join("profile.json")
}

fn read_profile_metadata(profile_dir: &Path) -> anyhow::Result<Option<ProfileMetadata>> {
    let path = profile_metadata_path(profile_dir);
    if !path.exists() {
        return Ok(None);
    }

    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

fn resolve_profile(switch_home: &Path, selector: &str) -> anyhow::Result<ResolvedProfile> {
    // 优先尝试新格式：直接按 id 匹配
    if let Some(flat) = read_flat_profile(switch_home, selector)? {
        let auth = serde_json::from_value(flat.auth)
            .with_context(|| format!("profile {selector} 中的 auth 数据格式无效"))?;
        return Ok(ResolvedProfile { id: selector.to_string(), name: flat.name, auth });
    }

    // 尝试旧格式目录（按 id 精确匹配），并迁移
    let root = profiles_root(switch_home);
    let legacy_path = root.join(selector);
    if legacy_path.is_dir() && legacy_path.join("auth.json").exists() {
        migrate_legacy_profile(switch_home, &legacy_path, selector)?;
        return resolve_profile(switch_home, selector);
    }

    // 按显示名搜索新格式文件和旧格式目录
    let mut matches: Vec<ResolvedProfile> = Vec::new();
    if root.exists() {
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let Some(id) = path.file_name().and_then(|v| v.to_str()) else {
                    continue;
                };
                if id.starts_with('.') || !path.join("auth.json").exists() {
                    continue;
                }
                let metadata =
                    read_profile_metadata(&path)?.unwrap_or_else(|| ProfileMetadata::with_name(id));
                if metadata.name == selector {
                    migrate_legacy_profile(switch_home, &path, id)?;
                    if let Some(flat) = read_flat_profile(switch_home, id)? {
                        let auth = serde_json::from_value(flat.auth)
                            .with_context(|| format!("profile {id} 中的 auth 数据格式无效"))?;
                        matches.push(ResolvedProfile { id: id.to_string(), name: flat.name, auth });
                    }
                }
                continue;
            }

            if !path.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if file_name == "state.json" || file_name.starts_with('.') {
                continue;
            }
            let Some(id) = file_name.strip_suffix(".json") else {
                continue;
            };
            let flat: FlatProfile = serde_json::from_slice(&fs::read(&path)?)?;
            if flat.name == selector {
                let auth = serde_json::from_value(flat.auth)
                    .with_context(|| format!("profile {id} 中的 auth 数据格式无效"))?;
                matches.push(ResolvedProfile { id: id.to_string(), name: flat.name, auth });
            }
        }
    }

    if matches.len() > 1 {
        bail!("存在多个同名 profile，请使用 id 切换: {selector}");
    }

    if let Some(profile) = matches.pop() {
        return Ok(profile);
    }

    bail!("profile 不存在: {selector}")
}

fn read_state(switch_home: &Path) -> anyhow::Result<ProfilesState> {
    let path = state_path(switch_home);
    if !path.exists() {
        return Ok(ProfilesState::default());
    }

    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn write_state(switch_home: &Path, state: &ProfilesState) -> anyhow::Result<()> {
    let root = profiles_root(switch_home);
    fs::create_dir_all(&root)?;
    fs::write(state_path(switch_home), serde_json::to_vec_pretty(state)?)?;
    Ok(())
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ProfilesState {
    active_profile: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ProfileMetadata {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary: Option<PrimaryRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary: Option<PrimaryRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_type: Option<String>,
}

impl ProfileMetadata {
    fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            primary: None,
            secondary: None,
            plan_type: None,
        }
    }
}

/// 扁平化存储格式：每个 profile 对应一个 `profiles/<id>.json` 文件，
/// 内嵌 auth 数据，避免每次 list 时解析 JWT。
#[derive(Debug, Clone, Deserialize, Serialize)]
struct FlatProfile {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription_plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary: Option<PrimaryRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary: Option<PrimaryRateLimit>,
    /// auth.json 完整内容，直接内嵌为 JSON 对象
    auth: Value,
}

fn flat_profile_path(switch_home: &Path, id: &str) -> PathBuf {
    profiles_root(switch_home).join(format!("{id}.json"))
}

fn read_flat_profile(switch_home: &Path, id: &str) -> anyhow::Result<Option<FlatProfile>> {
    let path = flat_profile_path(switch_home, id);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

fn write_flat_profile(switch_home: &Path, id: &str, profile: &FlatProfile) -> anyhow::Result<()> {
    let root = profiles_root(switch_home);
    fs::create_dir_all(&root)?;
    fs::write(flat_profile_path(switch_home, id), serde_json::to_vec_pretty(profile)?)?;
    Ok(())
}

/// 将旧格式目录（`profiles/<id>/auth.json` + `profile.json`）迁移为新扁平文件。
fn migrate_legacy_profile(switch_home: &Path, legacy_dir: &Path, id: &str) -> anyhow::Result<()> {
    let auth_path = legacy_dir.join("auth.json");
    if !auth_path.exists() {
        return Ok(());
    }
    let auth_file = auth::load_auth_file(&auth_path)?;
    let metadata = read_profile_metadata(legacy_dir)?.unwrap_or_else(|| ProfileMetadata::with_name(id));
    let summary = auth::build_account_summary_from_auth_file(auth_file.clone())?;
    let flat = FlatProfile {
        name: metadata.name,
        email: summary.email,
        subscription_plan: summary.subscription_plan,
        account_id: summary.account_id,
        plan_type: metadata.plan_type,
        primary: metadata.primary,
        secondary: metadata.secondary,
        auth: serde_json::to_value(auth_file)?,
    };
    write_flat_profile(switch_home, id, &flat)?;
    fs::remove_dir_all(legacy_dir)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct UsageLimitSnapshot {
    primary: Option<PrimaryRateLimit>,
    secondary: Option<PrimaryRateLimit>,
    plan_type: Option<String>,
}

fn current_usage_snapshot(codex_home: &Path) -> anyhow::Result<UsageLimitSnapshot> {
    let usage = sessions::collect_usage(codex_home)?;
    Ok(UsageLimitSnapshot {
        primary: usage.primary,
        secondary: usage.secondary,
        plan_type: usage.plan_type,
    })
}

fn refresh_active_profile_snapshot(codex_home: &Path, switch_home: &Path) -> anyhow::Result<()> {
    let state = read_state(switch_home)?;
    let Some(active_id) = state.active_profile else {
        return Ok(());
    };

    let Some(mut flat) = read_flat_profile(switch_home, &active_id)? else {
        return Ok(());
    };

    // 同步当前 auth.json（tokens 可能被 Codex 自动刷新）
    let current_auth_path = codex_home.join("auth.json");
    if current_auth_path.exists() {
        if let Ok(auth_file) = auth::load_auth_file(&current_auth_path) {
            if let Ok(summary) = auth::build_account_summary_from_auth_file(auth_file.clone()) {
                flat.email = summary.email.or(flat.email);
                flat.subscription_plan = summary.subscription_plan.or(flat.subscription_plan);
                flat.account_id = summary.account_id.or(flat.account_id);
            }
            flat.auth = serde_json::to_value(auth_file)?;
        }
    }

    let usage = current_usage_snapshot(codex_home)?;
    flat.primary = usage.primary;
    flat.secondary = usage.secondary;
    flat.plan_type = usage.plan_type;
    write_flat_profile(switch_home, &active_id, &flat)
}

struct ResolvedProfile {
    id: String,
    name: String,
    auth: auth::AuthFile,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::{delete_profiles, flat_profile_path, profiles_root, write_flat_profile, write_state, FlatProfile, ProfilesState};

    fn write_profile(switch_home: &std::path::Path, id: &str, name: &str) {
        write_flat_profile(
            switch_home,
            id,
            &FlatProfile {
                name: name.to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                auth: json!({}),
            },
        )
        .unwrap();
    }

    #[test]
    fn delete_profiles_removes_multiple_profiles() {
        let temp = tempdir().unwrap();
        let switch_home = temp.path();
        let root = profiles_root(switch_home);

        write_profile(switch_home, "alpha", "alpha");
        write_profile(switch_home, "beta", "beta");
        write_profile(switch_home, "gamma", "gamma");
        write_state(
            switch_home,
            &ProfilesState {
                active_profile: Some("gamma".to_string()),
            },
        )
        .unwrap();

        let message = delete_profiles(switch_home, &["alpha", "beta"]).unwrap();

        assert!(message.contains("已删除 2 个 profile"));
        assert!(!flat_profile_path(switch_home, "alpha").exists());
        assert!(!flat_profile_path(switch_home, "beta").exists());
        assert!(flat_profile_path(switch_home, "gamma").exists());

        let state: serde_json::Value = serde_json::from_slice(&fs::read(root.join("state.json")).unwrap()).unwrap();
        assert_eq!(state.get("active_profile").and_then(serde_json::Value::as_str), Some("gamma"));
    }

    #[test]
    fn delete_profiles_rejects_active_profile() {
        let temp = tempdir().unwrap();
        let switch_home = temp.path();

        write_profile(switch_home, "alpha", "alpha");
        write_profile(switch_home, "beta", "beta");
        write_state(
            switch_home,
            &ProfilesState {
                active_profile: Some("alpha".to_string()),
            },
        )
        .unwrap();

        let error = delete_profiles(switch_home, &["alpha"]).unwrap_err();

        assert!(error.to_string().contains("当前激活的 profile 不允许删除，请先切换到其他 profile"));
        assert!(flat_profile_path(switch_home, "alpha").exists());
    }

    #[test]
    fn resolve_profile_prefers_exact_id_before_duplicate_display_name() {
        let temp = tempdir().unwrap();
        let switch_home = temp.path();

        write_profile(switch_home, "ohanna27", "ohanna27");
        write_profile(switch_home, "ohanna27-2", "ohanna27");

        let resolved = super::resolve_profile(switch_home, "ohanna27").unwrap();

        assert_eq!(resolved.id, "ohanna27");
    }
}
