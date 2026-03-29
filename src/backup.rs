use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::{auth, model};

// ─────────────────────────────────────────────────────────────────────────────
// BackupConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackupConfig {
    pub webdav_url: String,
    pub webdav_user: String,
    pub webdav_password: String,
    pub remote_dir: String,
    pub max_backups: u32,
    pub encryption_password: Option<String>,
}

impl BackupConfig {
    fn config_path(switch_home: &Path) -> PathBuf {
        switch_home.join("backup.json")
    }

    pub fn load(switch_home: &Path) -> anyhow::Result<Option<Self>> {
        let path = Self::config_path(switch_home);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(
            serde_json::from_slice(&fs::read(&path)?)
                .with_context(|| format!("failed to parse backup config: {}", path.display()))?,
        ))
    }

    pub fn save(&self, switch_home: &Path) -> anyhow::Result<()> {
        let path = Self::config_path(switch_home);
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// zip 打包 / 解包
// ─────────────────────────────────────────────────────────────────────────────

pub fn pack_profiles_dir(profiles_dir: &Path) -> anyhow::Result<Vec<u8>> {
    use zip::write::{SimpleFileOptions, ZipWriter};

    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    pack_dir_into_zip(&mut zip, profiles_dir, profiles_dir, &options)?;

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

fn pack_dir_into_zip(
    zip: &mut zip::write::ZipWriter<std::io::Cursor<Vec<u8>>>,
    base: &Path,
    dir: &Path,
    options: &zip::write::SimpleFileOptions,
) -> anyhow::Result<()> {
    use std::io::Write as _;

    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            pack_dir_into_zip(zip, base, &path, options)?;
        } else {
            let relative = path.strip_prefix(base)?;
            let zip_path = relative.to_string_lossy().replace('\\', "/");
            zip.start_file(&zip_path, *options)?;
            zip.write_all(&fs::read(&path)?)?;
        }
    }
    Ok(())
}

pub struct BackupEntry {
    pub id: String,
    pub auth_bytes: Vec<u8>,
    pub metadata_bytes: Option<Vec<u8>>,
}

pub fn unpack_backup_entries(zip_bytes: &[u8]) -> anyhow::Result<Vec<BackupEntry>> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;

    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
        .collect::<Result<_, _>>()?;

    let mut profile_ids: Vec<String> = names
        .iter()
        .filter_map(|name| {
            let (prefix, suffix) = name.split_once('/')?;
            if suffix == "auth.json" && !prefix.starts_with('.') {
                Some(prefix.to_string())
            } else {
                None
            }
        })
        .collect();
    profile_ids.sort();
    profile_ids.dedup();

    let mut entries = Vec::new();
    for id in profile_ids {
        let auth_path = format!("{}/auth.json", id);
        let meta_path = format!("{}/profile.json", id);

        let auth_bytes = {
            let mut file = archive.by_name(&auth_path)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            bytes
        };
        let metadata_bytes = if names.iter().any(|n| n == &meta_path) {
            let mut file = archive.by_name(&meta_path)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Some(bytes)
        } else {
            None
        };

        entries.push(BackupEntry { id, auth_bytes, metadata_bytes });
    }

    Ok(entries)
}

// ─────────────────────────────────────────────────────────────────────────────
// AES-256-GCM 加密 / 解密
// ─────────────────────────────────────────────────────────────────────────────

const CSBK_MAGIC: &[u8; 4] = b"CSBK";
const PBKDF2_ITERATIONS: u32 = 100_000;

fn derive_key(password: &str, salt: &[u8; 16]) -> [u8; 32] {
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

pub fn encrypt(data: &[u8], password: &str) -> anyhow::Result<Vec<u8>> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };
    use rand::{rngs::OsRng, RngCore};

    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    let key_bytes = derive_key(password, &salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| anyhow::anyhow!("加密失败: {e}"))?;

    let mut result = Vec::with_capacity(4 + 16 + 12 + ciphertext.len());
    result.extend_from_slice(CSBK_MAGIC);
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt(data: &[u8], password: &str) -> anyhow::Result<Vec<u8>> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };

    if data.len() < 4 + 16 + 12 {
        bail!("加密文件格式无效：数据过短");
    }
    if &data[..4] != CSBK_MAGIC {
        bail!("加密文件格式无效：magic 不匹配");
    }
    let salt: [u8; 16] = data[4..20].try_into().unwrap();
    let nonce_bytes: [u8; 12] = data[20..32].try_into().unwrap();
    let ciphertext = &data[32..];

    let key_bytes = derive_key(password, &salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("解密失败，请检查密码"))
}

// ─────────────────────────────────────────────────────────────────────────────
// WebDAV HTTP 操作
// ─────────────────────────────────────────────────────────────────────────────

fn webdav_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("failed to build HTTP client")
}

fn webdav_file_url(config: &BackupConfig, filename: &str) -> String {
    let base = config.webdav_url.trim_end_matches('/');
    let dir = config.remote_dir.trim_matches('/');
    format!("{}/{}/{}", base, dir, filename)
}

fn webdav_dir_url(config: &BackupConfig) -> String {
    let base = config.webdav_url.trim_end_matches('/');
    let dir = config.remote_dir.trim_matches('/');
    format!("{}/{}/", base, dir)
}

pub fn webdav_mkcol(config: &BackupConfig) -> anyhow::Result<()> {
    let client = webdav_client();
    let resp = client
        .request(
            reqwest::Method::from_bytes(b"MKCOL").unwrap(),
            webdav_dir_url(config),
        )
        .basic_auth(&config.webdav_user, Some(&config.webdav_password))
        .send()
        .context("WebDAV MKCOL 请求失败")?;

    match resp.status().as_u16() {
        // 201 Created, 405 Method Not Allowed (collection already exists)
        201 | 405 => Ok(()),
        409 => bail!(
            "WebDAV MKCOL 失败，状态码 409（路径冲突）\n\
             常见原因：\n\
             1. WebDAV URL 中间路径不存在（Nextcloud 应为 .../remote.php/dav/files/用户名/）\n\
             2. 远端目录包含多层路径而中间目录尚未创建\n\
             请用 --setup 重新检查配置"
        ),
        code => bail!("WebDAV MKCOL 失败，状态码: {}", code),
    }
}

pub fn webdav_put(config: &BackupConfig, filename: &str, data: &[u8]) -> anyhow::Result<()> {
    let client = webdav_client();
    let resp = client
        .put(webdav_file_url(config, filename))
        .basic_auth(&config.webdav_user, Some(&config.webdav_password))
        .body(data.to_vec())
        .send()
        .context("WebDAV PUT 请求失败")?;

    let code = resp.status().as_u16();
    if matches!(code, 200 | 201 | 204) {
        Ok(())
    } else if code == 409 {
        bail!(
            "WebDAV PUT 失败，状态码 409（路径冲突）\n\
             远端目录可能不存在，请检查 WebDAV URL 和远端目录配置\n\
             使用 --setup 重新配置"
        )
    } else {
        bail!("WebDAV PUT 失败，状态码: {}", code)
    }
}

pub fn webdav_get(config: &BackupConfig, filename: &str) -> anyhow::Result<Vec<u8>> {
    let client = webdav_client();
    let resp = client
        .get(webdav_file_url(config, filename))
        .basic_auth(&config.webdav_user, Some(&config.webdav_password))
        .send()
        .context("WebDAV GET 请求失败")?;

    let code = resp.status().as_u16();
    if code != 200 {
        bail!("WebDAV GET 失败，状态码: {}", code);
    }
    Ok(resp.bytes().context("读取响应体失败")?.to_vec())
}

pub fn webdav_delete(config: &BackupConfig, filename: &str) -> anyhow::Result<()> {
    let client = webdav_client();
    let resp = client
        .delete(webdav_file_url(config, filename))
        .basic_auth(&config.webdav_user, Some(&config.webdav_password))
        .send()
        .context("WebDAV DELETE 请求失败")?;

    let code = resp.status().as_u16();
    if matches!(code, 200 | 204) {
        Ok(())
    } else {
        bail!("WebDAV DELETE 失败，状态码: {}", code)
    }
}

pub fn webdav_list_backups(config: &BackupConfig) -> anyhow::Result<Vec<String>> {
    let client = webdav_client();
    let resp = client
        .request(
            reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
            webdav_dir_url(config),
        )
        .basic_auth(&config.webdav_user, Some(&config.webdav_password))
        .header("Depth", "1")
        .header("Content-Type", "application/xml")
        .body(
            r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:displayname/></d:prop></d:propfind>"#,
        )
        .send()
        .context("WebDAV PROPFIND 请求失败")?;

    let code = resp.status().as_u16();
    if code != 207 && code != 200 {
        bail!("WebDAV PROPFIND 失败，状态码: {}", code);
    }

    let xml = resp.text().context("读取 PROPFIND 响应失败")?;
    let hrefs = parse_propfind_hrefs(&xml);
    let mut files: Vec<String> = hrefs
        .iter()
        .filter_map(|href| href_to_filename(href))
        .filter(|name| name.starts_with("codex-switch-"))
        .collect();
    files.sort();
    Ok(files)
}

fn parse_propfind_hrefs(xml: &str) -> Vec<String> {
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_href = false;
    let mut hrefs = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.local_name().as_ref() == b"href" {
                    in_href = true;
                }
            }
            Ok(Event::Text(ref e)) if in_href => {
                if let Ok(text) = e.unescape() {
                    hrefs.push(text.into_owned());
                }
                in_href = false;
            }
            Ok(Event::End(_)) => {
                in_href = false;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    hrefs
}

fn href_to_filename(href: &str) -> Option<String> {
    if href.ends_with('/') {
        return None;
    }
    href.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(String::from)
}

// ─────────────────────────────────────────────────────────────────────────────
// BackupEntry: to_profile_summary + write_backup_profile
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct BackupProfileMetadata {
    name: String,
}

impl BackupEntry {
    pub fn to_profile_summary(&self) -> anyhow::Result<model::ProfileSummary> {
        let metadata: Option<BackupProfileMetadata> = self
            .metadata_bytes
            .as_deref()
            .and_then(|b| serde_json::from_slice(b).ok());
        let name = metadata.map(|m| m.name).unwrap_or_else(|| self.id.clone());

        let auth_file: auth::AuthFile = serde_json::from_slice(&self.auth_bytes)
            .with_context(|| format!("无法解析 profile {} 的 auth.json", self.id))?;
        let summary = auth::build_account_summary_from_auth_file(auth_file)?;

        Ok(model::ProfileSummary {
            id: self.id.clone(),
            name,
            email: summary.email,
            subscription_plan: summary.subscription_plan,
            account_id: summary.account_id,
            plan_type: None,
            primary: None,
            secondary: None,
            active: false,
        })
    }
}

/// 将备份 entry 写入本地 profiles 目录。
/// 返回 `true` = 已跳过（本地同 id 已存在），`false` = 成功写入。
pub fn write_backup_profile(switch_home: &Path, entry: &BackupEntry) -> anyhow::Result<bool> {
    let profile_dir = switch_home.join("profiles").join(&entry.id);
    if profile_dir.exists() {
        return Ok(true);
    }
    fs::create_dir_all(&profile_dir)?;

    let auth_file: auth::AuthFile = serde_json::from_slice(&entry.auth_bytes)
        .with_context(|| format!("无法解析备份 profile {} 的 auth.json", entry.id))?;
    auth::write_auth_file(&profile_dir.join("auth.json"), &auth_file)?;

    if let Some(meta_bytes) = &entry.metadata_bytes {
        fs::write(profile_dir.join("profile.json"), meta_bytes)?;
    }

    Ok(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// run_backup / run_restore 编排
// ─────────────────────────────────────────────────────────────────────────────

fn backup_filename(encrypted: bool) -> String {
    let now = chrono::Local::now();
    let ext = if encrypted { "zip.enc" } else { "zip" };
    format!("codex-switch-{}.{}", now.format("%Y%m%d-%H%M%S"), ext)
}

fn default_config() -> BackupConfig {
    BackupConfig {
        webdav_url: String::new(),
        webdav_user: String::new(),
        webdav_password: String::new(),
        remote_dir: "codex-switch-backups/".to_string(),
        max_backups: 10,
        encryption_password: None,
    }
}

fn config_fields(config: &BackupConfig) -> Vec<(&'static str, String, bool, &'static str)> {
    vec![
        (
            "WebDAV URL",
            config.webdav_url.clone(),
            false,
            "WebDAV 服务器地址，需以 / 结尾\n例: https://your-server/dav/",
        ),
        (
            "用户名",
            config.webdav_user.clone(),
            false,
            "WebDAV 登录用户名",
        ),
        (
            "密码",
            config.webdav_password.clone(),
            true,
            "WebDAV 登录密码（输入时以 ● 掩码显示）",
        ),
        (
            "远端目录",
            config.remote_dir.clone(),
            false,
            "服务器端备份存放目录，需以 / 结尾\n默认: codex-switch-backups/",
        ),
        (
            "最多备份数",
            config.max_backups.to_string(),
            false,
            "服务器保留的最大备份数量\n0 = 不限制，默认 10",
        ),
        (
            "加密口令",
            config.encryption_password.clone().unwrap_or_default(),
            true,
            "AES-256-GCM 加密备份口令（以 ● 掩码显示）\n留空 = 明文 .zip\n填写后生成 .zip.enc",
        ),
    ]
}

fn config_from_values(values: Vec<String>) -> anyhow::Result<BackupConfig> {
    let max_backups: u32 = values[4].trim().parse().unwrap_or(10);
    let enc_password = if values[5].trim().is_empty() {
        None
    } else {
        Some(values[5].clone())
    };
    let raw_dir = values[3].trim_start_matches('/').to_string();
    let remote_dir = if raw_dir.trim().is_empty() {
        "codex-switch-backups/".to_string()
    } else if raw_dir.ends_with('/') {
        raw_dir
    } else {
        format!("{}/", raw_dir)
    };
    Ok(BackupConfig {
        webdav_url: values[0].trim().to_string(),
        webdav_user: values[1].trim().to_string(),
        webdav_password: values[2].clone(),
        remote_dir,
        max_backups,
        encryption_password: enc_password,
    })
}

/// 加载已有配置，或（首次/--setup 时）弹出 TUI 向导让用户填写。
/// `action_label` 用于取消时的提示，如 "备份" 或 "恢复"。
fn load_or_setup_config(
    switch_home: &Path,
    setup: bool,
    action_label: &str,
) -> anyhow::Result<Option<BackupConfig>> {
    let existing = BackupConfig::load(switch_home)?;
    if !setup {
        if let Some(cfg) = existing {
            return Ok(Some(cfg));
        }
    }
    // 首次运行或 --setup：弹向导
    let title = if setup {
        if action_label == "备份" { "Backup Config" } else { "Restore Config" }
    } else {
        if action_label == "备份" { "Backup 初始配置" } else { "Restore 初始配置" }
    };
    let base = existing.unwrap_or_else(default_config);
    let fields = config_fields(&base);
    let Some(values) = crate::tui::edit_config_fields(title, fields)? else {
        return Ok(None);
    };
    let cfg = config_from_values(values)?;
    cfg.save(switch_home)?;
    Ok(Some(cfg))
}

pub fn run_backup(switch_home: &Path, setup: bool) -> anyhow::Result<String> {
    let Some(config) = load_or_setup_config(switch_home, setup, "备份")? else {
        return Ok("已取消备份".to_string());
    };

    let profiles_dir = switch_home.join("profiles");
    if !profiles_dir.exists() {
        bail!("profiles 目录不存在，请先保存一个 profile");
    }

    let zip_bytes = pack_profiles_dir(&profiles_dir)?;

    let (payload, filename) = if let Some(ref password) = config.encryption_password {
        let encrypted = encrypt(&zip_bytes, password)?;
        (encrypted, backup_filename(true))
    } else {
        (zip_bytes, backup_filename(false))
    };

    let size_kb = payload.len().saturating_add(1023) / 1024;
    webdav_mkcol(&config)?;
    webdav_put(&config, &filename, &payload)?;

    if config.max_backups > 0 {
        let mut files = webdav_list_backups(&config)?;
        files.sort();
        let keep = config.max_backups as usize;
        if files.len() > keep {
            let to_delete = files[..files.len() - keep].to_vec();
            for old in to_delete {
                webdav_delete(&config, &old)?;
            }
        }
    }

    Ok(format!("备份成功: {} ({} KB)", filename, size_kb))
}

pub fn run_restore(switch_home: &Path, setup: bool) -> anyhow::Result<String> {
    let Some(config) = load_or_setup_config(switch_home, setup, "恢复")? else {
        return Ok("已取消恢复".to_string());
    };

    let mut files = webdav_list_backups(&config)?;
    if files.is_empty() {
        return Ok("WebDAV 目录中暂无备份文件".to_string());
    }
    files.sort_by(|a, b| b.cmp(a));

    let Some(selected_file) = crate::tui::select_backup_file(files)? else {
        return Ok("已取消恢复".to_string());
    };

    let raw = webdav_get(&config, &selected_file)?;

    let zip_bytes = if selected_file.ends_with(".enc") {
        let password = if let Some(p) = config.encryption_password.clone() {
            p
        } else {
            let Some(pw) = crate::tui::input_password("请输入解密口令")? else {
                return Ok("已取消恢复".to_string());
            };
            pw
        };
        decrypt(&raw, &password)?
    } else {
        raw
    };

    let entries = unpack_backup_entries(&zip_bytes)?;
    if entries.is_empty() {
        return Ok("备份中不含任何 profile".to_string());
    }

    let profiles_dir = switch_home.join("profiles");
    let existing_ids: HashSet<String> = if profiles_dir.exists() {
        fs::read_dir(&profiles_dir)?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.'))
            .collect()
    } else {
        HashSet::new()
    };

    let summaries: Vec<model::ProfileSummary> = entries
        .iter()
        .filter_map(|e| e.to_profile_summary().ok())
        .collect();

    let selected = crate::tui::select_backup_profiles(summaries, existing_ids)?;
    let Some(to_import) = selected else {
        return Ok("已取消恢复".to_string());
    };

    if to_import.is_empty() {
        return Ok("未选择任何 profile".to_string());
    }

    let mut imported = 0usize;
    let mut skipped = 0usize;
    for profile in &to_import {
        if let Some(entry) = entries.iter().find(|e| e.id == profile.id) {
            if write_backup_profile(switch_home, entry)? {
                skipped += 1;
            } else {
                imported += 1;
            }
        }
    }

    Ok(format!(
        "恢复完成：导入 {} 个，跳过(已存在) {} 个",
        imported, skipped
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_config(base_url: &str) -> BackupConfig {
        BackupConfig {
            webdav_url: base_url.to_string(),
            webdav_user: "u".to_string(),
            webdav_password: "p".to_string(),
            remote_dir: "bk/".to_string(),
            max_backups: 10,
            encryption_password: None,
        }
    }

    #[test]
    fn backup_config_roundtrip() {
        let temp = tempdir().unwrap();
        let config = BackupConfig {
            webdav_url: "https://dav.example.com/".to_string(),
            webdav_user: "user".to_string(),
            webdav_password: "pass".to_string(),
            remote_dir: "backups/".to_string(),
            max_backups: 5,
            encryption_password: Some("secret".to_string()),
        };
        config.save(temp.path()).unwrap();
        let loaded = BackupConfig::load(temp.path()).unwrap().unwrap();
        assert_eq!(loaded.webdav_url, "https://dav.example.com/");
        assert_eq!(loaded.max_backups, 5);
        assert_eq!(loaded.encryption_password, Some("secret".to_string()));
    }

    #[test]
    fn backup_config_load_returns_none_when_missing() {
        let temp = tempdir().unwrap();
        assert!(BackupConfig::load(temp.path()).unwrap().is_none());
    }

    #[test]
    fn pack_profiles_creates_zip_with_all_files() {
        let temp = tempdir().unwrap();
        let profiles_dir = temp.path();

        fs::create_dir_all(profiles_dir.join("alice")).unwrap();
        fs::write(profiles_dir.join("alice/auth.json"), b"{}").unwrap();
        fs::write(profiles_dir.join("alice/profile.json"), b"{\"name\":\"alice\"}").unwrap();
        fs::write(profiles_dir.join("state.json"), b"{\"active_profile\":\"alice\"}").unwrap();
        fs::create_dir_all(profiles_dir.join(".rollback")).unwrap();
        fs::write(profiles_dir.join(".rollback/auth.json"), b"{}").unwrap();

        let zip_bytes = pack_profiles_dir(profiles_dir).unwrap();
        assert!(!zip_bytes.is_empty());

        let cursor = std::io::Cursor::new(&zip_bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        assert!(
            names.iter().any(|n| n == "alice/auth.json"),
            "missing alice/auth.json, got: {:?}",
            names
        );
        assert!(names.iter().any(|n| n == "alice/profile.json"));
        assert!(names.iter().any(|n| n == "state.json"));
        assert!(
            !names.iter().any(|n| n.contains(".rollback")),
            "hidden entries should be excluded"
        );
    }

    #[test]
    fn unpack_backup_entries_identifies_profiles() {
        let temp = tempdir().unwrap();
        let profiles_dir = temp.path();
        fs::create_dir_all(profiles_dir.join("alice")).unwrap();
        fs::write(profiles_dir.join("alice/auth.json"), b"{\"test\":1}").unwrap();
        fs::write(profiles_dir.join("alice/profile.json"), b"{\"name\":\"Alice\"}").unwrap();
        fs::create_dir_all(profiles_dir.join("bob")).unwrap();
        fs::write(profiles_dir.join("bob/auth.json"), b"{\"test\":2}").unwrap();

        let zip_bytes = pack_profiles_dir(profiles_dir).unwrap();
        let entries = unpack_backup_entries(&zip_bytes).unwrap();

        assert_eq!(entries.len(), 2);
        let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"alice"));
        assert!(ids.contains(&"bob"));

        let alice = entries.iter().find(|e| e.id == "alice").unwrap();
        assert_eq!(alice.auth_bytes, b"{\"test\":1}");
        assert!(alice.metadata_bytes.is_some());
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let original = b"hello, world!";
        let encrypted = encrypt(original, "my-password").unwrap();
        assert_ne!(&encrypted[..], original as &[u8]);
        assert_eq!(&encrypted[..4], b"CSBK");

        let decrypted = decrypt(&encrypted, "my-password").unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn decrypt_fails_with_wrong_password() {
        let encrypted = encrypt(b"secret data", "correct-pass").unwrap();
        let result = decrypt(&encrypted, "wrong-pass");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("解密失败"));
    }

    #[test]
    fn decrypt_fails_with_invalid_data() {
        let result = decrypt(b"not-encrypted", "any-password");
        assert!(result.is_err());
    }

    #[test]
    fn parse_propfind_hrefs_extracts_hrefs() {
        let xml = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:">
  <d:response><d:href>/bk/</d:href></d:response>
  <d:response><d:href>/bk/codex-switch-20260329-100000.zip</d:href></d:response>
  <d:response><d:href>/bk/codex-switch-20260328-090000.zip</d:href></d:response>
</d:multistatus>"#;
        let hrefs = parse_propfind_hrefs(xml);
        assert_eq!(hrefs.len(), 3);
    }

    #[test]
    fn href_to_filename_extracts_last_segment() {
        assert_eq!(
            href_to_filename("/bk/codex-switch-20260329.zip"),
            Some("codex-switch-20260329.zip".to_string())
        );
        assert_eq!(href_to_filename("/bk/"), None);
    }

    #[test]
    fn backup_entry_to_profile_summary_parses_name_and_email() {
        let auth_json = r#"{
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": null,
                "access_token": null,
                "account_id": "acct-123"
            }
        }"#;
        let meta_json = r#"{"name": "Alice"}"#;

        let entry = BackupEntry {
            id: "alice".to_string(),
            auth_bytes: auth_json.as_bytes().to_vec(),
            metadata_bytes: Some(meta_json.as_bytes().to_vec()),
        };

        let summary = entry.to_profile_summary().unwrap();
        assert_eq!(summary.id, "alice");
        assert_eq!(summary.name, "Alice");
        assert_eq!(summary.account_id, Some("acct-123".to_string()));
        assert!(!summary.active);
    }

    #[test]
    fn write_backup_profile_creates_files() {
        let temp = tempdir().unwrap();
        let switch_home = temp.path();
        let auth_json = r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"account_id":"x"}}"#;
        let meta_json = r#"{"name":"test"}"#;

        let entry = BackupEntry {
            id: "test-profile".to_string(),
            auth_bytes: auth_json.as_bytes().to_vec(),
            metadata_bytes: Some(meta_json.as_bytes().to_vec()),
        };

        let skipped = write_backup_profile(switch_home, &entry).unwrap();
        assert!(!skipped);

        let profile_dir = switch_home.join("profiles/test-profile");
        assert!(profile_dir.join("auth.json").exists());
        assert!(profile_dir.join("profile.json").exists());
    }

    #[test]
    fn write_backup_profile_skips_existing_id() {
        let temp = tempdir().unwrap();
        let switch_home = temp.path();
        let profile_dir = switch_home.join("profiles/existing");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(profile_dir.join("auth.json"), b"original").unwrap();

        let entry = BackupEntry {
            id: "existing".to_string(),
            auth_bytes: b"new-content".to_vec(),
            metadata_bytes: None,
        };

        let skipped = write_backup_profile(switch_home, &entry).unwrap();
        assert!(skipped);
        assert_eq!(
            fs::read(profile_dir.join("auth.json")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn webdav_put_and_get_roundtrip() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let put_data = b"hello-backup-data";

        let _put_mock = server.mock(|when, then| {
            when.method(PUT).path("/bk/testfile.zip");
            then.status(201);
        });
        let _get_mock = server.mock(|when, then| {
            when.method(GET).path("/bk/testfile.zip");
            then.status(200).body(put_data.as_ref());
        });

        let config = make_config(&server.url("/"));
        webdav_put(&config, "testfile.zip", put_data).unwrap();
        let got = webdav_get(&config, "testfile.zip").unwrap();
        assert_eq!(got, put_data);
    }

    #[test]
    fn webdav_list_backups_filters_non_backup_hrefs() {
        let xml = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:">
  <d:response><d:href>/bk/</d:href></d:response>
  <d:response><d:href>/bk/codex-switch-20260329-100000.zip</d:href></d:response>
  <d:response><d:href>/bk/codex-switch-20260328-090000.zip</d:href></d:response>
  <d:response><d:href>/bk/other-file.txt</d:href></d:response>
</d:multistatus>"#;

        let files: Vec<String> = parse_propfind_hrefs(xml)
            .into_iter()
            .filter_map(|href| href_to_filename(&href))
            .filter(|name| name.starts_with("codex-switch-"))
            .collect();

        assert_eq!(files.len(), 2);
        assert!(files.contains(&"codex-switch-20260329-100000.zip".to_string()));
        assert!(files.contains(&"codex-switch-20260328-090000.zip".to_string()));
    }
}
