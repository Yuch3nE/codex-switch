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

    let summary = auth::build_account_summary_from_path(&current_auth)?;
    let display_name = resolve_display_name(name, summary.email.as_deref())?;
    let profile_id = generate_profile_id(switch_home, &display_name)?;

    let profile_dir = profiles_root(switch_home).join(&profile_id);
    fs::create_dir_all(&profile_dir)?;
    fs::copy(&current_auth, profile_dir.join("auth.json"))?;
    let primary = current_primary_snapshot(codex_home)?;
    write_profile_metadata(
        &profile_dir,
        &ProfileMetadata {
            name: display_name.clone(),
            primary,
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
    let profile_auth = resolved.path.join("auth.json");

    let current_auth = codex_home.join("auth.json");
    let rollback_dir = profiles_root(switch_home).join(".rollback");
    fs::create_dir_all(&rollback_dir)?;

    if current_auth.exists() {
        fs::copy(&current_auth, rollback_dir.join("auth.json"))?;
    }

    fs::copy(&profile_auth, &current_auth)?;
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

pub fn list_profiles(codex_home: &Path, switch_home: &Path) -> anyhow::Result<ProfileListOutput> {
    let root = profiles_root(switch_home);
    fs::create_dir_all(&root)?;
    let state = read_state(switch_home)?;
    let active_primary = current_primary_snapshot(codex_home)?;
    let mut profiles = Vec::new();

    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let id = match path.file_name().and_then(|value| value.to_str()) {
            Some(name) if !name.starts_with('.') => name.to_string(),
            _ => continue,
        };

        let auth_path = path.join("auth.json");
        if !auth_path.exists() {
            continue;
        }

        let metadata = read_profile_metadata(&path)?.unwrap_or(ProfileMetadata {
            name: id.clone(),
            primary: None,
        });

        let summary = auth::build_account_summary_from_path(&auth_path)
            .with_context(|| format!("failed to read profile {id}"))?;

        let is_active = state.active_profile.as_deref() == Some(id.as_str());

        profiles.push(ProfileSummary {
            active: is_active,
            id,
            name: metadata.name,
            email: summary.email,
            subscription_plan: summary.subscription_plan,
            account_id: summary.account_id,
            primary: if is_active {
                active_primary.clone().or(metadata.primary)
            } else {
                metadata.primary
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

fn collect_auth_files(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_auth_files(&path, files)?;
        } else if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name == "auth.json")
        {
            files.push(path);
        }
    }

    files.sort();
    Ok(())
}

fn collect_cpa_files(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_cpa_files(&path, files)?;
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            files.push(path);
        }
    }

    files.sort();
    Ok(())
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
    let profile_dir = profiles_root(switch_home).join(profile_id);

    fs::create_dir_all(&profile_dir)?;
    fs::write(
        profile_dir.join("auth.json"),
        serde_json::to_vec_pretty(&auth_file)?,
    )?;
    write_profile_metadata(
        &profile_dir,
        &ProfileMetadata {
            name: display_name,
            primary: None,
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
        tokens: auth::AuthTokens {
            id_token: Some(cpa.id_token),
            access_token: Some(cpa.access_token),
            account_id: Some(cpa.account_id),
        },
        refresh_token: cpa.refresh_token,
        last_refresh: cpa.last_refresh,
    })
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

    while root.join(&candidate).exists() {
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

fn write_profile_metadata(profile_dir: &Path, metadata: &ProfileMetadata) -> anyhow::Result<()> {
    fs::write(
        profile_metadata_path(profile_dir),
        serde_json::to_vec_pretty(metadata)?,
    )?;
    Ok(())
}

fn resolve_profile(switch_home: &Path, selector: &str) -> anyhow::Result<ResolvedProfile> {
    let root = profiles_root(switch_home);
    let mut matches = Vec::new();
    if root.exists() {
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let Some(id) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if id.starts_with('.') || !path.join("auth.json").exists() {
                continue;
            }

            let metadata = read_profile_metadata(&path)?.unwrap_or(ProfileMetadata {
                name: id.to_string(),
                primary: None,
            });
            if metadata.name == selector {
                matches.push(ResolvedProfile {
                    id: id.to_string(),
                    name: metadata.name,
                    path,
                });
            }
        }
    }

    if matches.len() > 1 {
        bail!("存在多个同名 profile，请使用 id 切换: {selector}");
    }

    if let Some(profile) = matches.pop() {
        return Ok(profile);
    }

    let direct_path = root.join(selector);
    if direct_path.join("auth.json").exists() {
        let metadata = read_profile_metadata(&direct_path)?.unwrap_or(ProfileMetadata {
            name: selector.to_string(),
            primary: None,
        });
        return Ok(ResolvedProfile {
            id: selector.to_string(),
            name: metadata.name,
            path: direct_path,
        });
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
}

fn current_primary_snapshot(codex_home: &Path) -> anyhow::Result<Option<PrimaryRateLimit>> {
    Ok(sessions::collect_usage(codex_home)?.primary)
}

fn refresh_active_profile_snapshot(codex_home: &Path, switch_home: &Path) -> anyhow::Result<()> {
    let state = read_state(switch_home)?;
    let Some(active_profile) = state.active_profile else {
        return Ok(());
    };

    let profile_dir = profiles_root(switch_home).join(active_profile);
    if !profile_dir.join("auth.json").exists() {
        return Ok(());
    }

    let mut metadata = read_profile_metadata(&profile_dir)?.unwrap_or(ProfileMetadata {
        name: profile_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("profile")
            .to_string(),
        primary: None,
    });
    metadata.primary = current_primary_snapshot(codex_home)?;
    write_profile_metadata(&profile_dir, &metadata)
}

struct ResolvedProfile {
    id: String,
    name: String,
    path: PathBuf,
}
