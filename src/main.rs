mod auth;
mod backup;
mod cli;
mod jwt;
mod model;
mod profiles;
mod sessions;
mod tui;

use std::fs;
use std::path::PathBuf;
use std::io::IsTerminal;

use anyhow::Context;
use clap::Parser;

struct AppPaths {
    codex_home: PathBuf,
    switch_home: PathBuf,
}

fn main() {
    if let Err(err) = run() {
        let msg = format!("{err:#}");
        eprintln!("错误: {msg}");
        let code = if msg.contains("E_NOT_FOUND:") {
            2
        } else if msg.contains("E_INTERACTIVE_REQUIRED:") {
            3
        } else if msg.contains("E_AMBIGUOUS_SELECTOR:") {
            4
        } else {
            1
        };
        std::process::exit(code);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let allow_interactive = std::io::stdout().is_terminal() && !cli.non_interactive;
    let format = cli.format;

    if let cli::Command::Completions { shell } = &cli.command {
        use clap::CommandFactory;
        clap_complete::generate(
            *shell,
            &mut cli::Cli::command(),
            "codex-switch",
            &mut std::io::stdout(),
        );
        return Ok(());
    }

    let paths = resolve_app_paths()?;

    let output = match cli.command {
        cli::Command::Account => auth::build_account_summary(&paths.codex_home)?.render(format)?,
        cli::Command::Doctor => build_doctor_output(&paths)?.render(format)?,
        cli::Command::Usage => {
            let profiles = profiles::list_profiles(&paths.codex_home, &paths.switch_home)?;
            model::UsageTableOutput::from_profiles(profiles).render(format)?
        }
        cli::Command::Profile { command } => match command {
            cli::ProfileCommand::Save { name } => {
                profiles::save_profile(&paths.codex_home, &paths.switch_home, name.as_deref())?
                    .render(format)?
            }
            cli::ProfileCommand::Use { name, auto } => {
                if auto {
                    let profiles = profiles::list_profiles(&paths.codex_home, &paths.switch_home)?;
                    if profiles.profiles.is_empty() {
                        anyhow::bail!(
                            "E_NOT_FOUND: 暂无 profiles，请先使用 profile save 保存当前账号"
                        );
                    } else if let Some(best) = profiles.best_profile() {
                        if profiles.active_profile.as_deref() == Some(best.id.as_str()) {
                            model::MutationResult {
                                ok: true,
                                action: "use".to_string(),
                                id: Some(best.id.clone()),
                                name: Some(best.name.clone()),
                                email: best.email.clone(),
                                ids: None,
                                count: None,
                                message: format!(
                                    "当前已是最优 profile: {} ({})",
                                    best.name,
                                    best.email.as_deref().unwrap_or("")
                                ),
                            }
                            .render(format)?
                        } else {
                            profiles::use_profile(
                                &paths.codex_home,
                                &paths.switch_home,
                                &best.id,
                            )?
                            .render(format)?
                        }
                    } else {
                        anyhow::bail!(
                            "E_NOT_FOUND: 暂无 profiles，请先使用 profile save 保存当前账号"
                        );
                    }
                } else {
                    match name {
                        Some(name) => {
                            let profiles = profiles::list_profiles(
                                &paths.codex_home,
                                &paths.switch_home,
                            )?;
                            let matches = matching_profiles_by_selector(&profiles, &name);

                            if matches.len() > 1 {
                                if !allow_interactive {
                                    anyhow::bail!(
                                        "E_AMBIGUOUS_SELECTOR: 存在多个匹配 profile，请使用 id 切换: {name}"
                                    );
                                }
                                let duplicate_output = model::ProfileListOutput {
                                    active_profile: profiles.active_profile.clone(),
                                    profiles: matches,
                                };
                                select_and_use_profile(
                                    &paths.codex_home,
                                    &paths.switch_home,
                                    duplicate_output,
                                )?
                                .render(format)?
                            } else if let Some(single) = matches.into_iter().next() {
                                profiles::use_profile(
                                    &paths.codex_home,
                                    &paths.switch_home,
                                    &single.id,
                                )?
                                .render(format)?
                            } else {
                                profiles::use_profile(
                                    &paths.codex_home,
                                    &paths.switch_home,
                                    &name,
                                )?
                                .render(format)?
                            }
                        }
                        None => {
                            let profiles = profiles::list_profiles(
                                &paths.codex_home,
                                &paths.switch_home,
                            )?;
                            if profiles.profiles.is_empty() {
                                anyhow::bail!(
                                    "E_NOT_FOUND: 暂无 profiles，请先使用 profile save 保存当前账号"
                                );
                            } else if !allow_interactive {
                                anyhow::bail!(
                                    "E_INTERACTIVE_REQUIRED: 未提供 profile 选择器，且当前为非交互模式；请传 name/email/id 或 --auto"
                                )
                            } else {
                                select_and_use_profile(
                                    &paths.codex_home,
                                    &paths.switch_home,
                                    profiles,
                                )?
                                .render(format)?
                            }
                        }
                    }
                }
            }
            cli::ProfileCommand::Delete { name } => {
                let profiles = profiles::list_profiles(&paths.codex_home, &paths.switch_home)?;
                if profiles.profiles.is_empty() {
                    anyhow::bail!(
                        "E_NOT_FOUND: 暂无 profiles，请先使用 profile save 保存当前账号"
                    );
                } else if let Some(selector) = name {
                    let matches = matching_profiles_by_selector(&profiles, &selector);
                    if matches.is_empty() {
                        anyhow::bail!("E_NOT_FOUND: 未找到匹配的 profile: {selector}");
                    } else if matches.len() == 1 {
                        let id = matches[0].id.clone();
                        profiles::delete_profiles(&paths.switch_home, &[id.as_str()])?
                            .render(format)?
                    } else {
                        if !allow_interactive {
                            anyhow::bail!(
                                "E_AMBIGUOUS_SELECTOR: 存在多个匹配 profile，请使用 id 指定: {selector}"
                            );
                        }
                        let candidates = model::ProfileListOutput {
                            active_profile: profiles.active_profile.clone(),
                            profiles: matches,
                        };
                        if let Some(selected) = tui::select_profiles_to_delete(candidates)? {
                            let selected_ids =
                                selected.iter().map(|p| p.id.as_str()).collect::<Vec<_>>();
                            profiles::delete_profiles(&paths.switch_home, &selected_ids)?
                                .render(format)?
                        } else {
                            model::MutationResult {
                                ok: false,
                                action: "cancel".to_string(),
                                id: None,
                                name: None,
                                email: None,
                                ids: None,
                                count: None,
                                message: "已取消删除".to_string(),
                            }
                            .render(format)?
                        }
                    }
                } else if !allow_interactive {
                    anyhow::bail!(
                        "E_INTERACTIVE_REQUIRED: 未提供删除选择器，且当前为非交互模式；请传 name/email/id"
                    )
                } else if let Some(selected) = tui::select_profiles_to_delete(profiles)? {
                    let selected_ids =
                        selected.iter().map(|profile| profile.id.as_str()).collect::<Vec<_>>();
                    profiles::delete_profiles(&paths.switch_home, &selected_ids)?
                        .render(format)?
                } else {
                    model::MutationResult {
                        ok: false,
                        action: "cancel".to_string(),
                        id: None,
                        name: None,
                        email: None,
                        ids: None,
                        count: None,
                        message: "已取消删除".to_string(),
                    }
                    .render(format)?
                }
            }
            cli::ProfileCommand::Backup { setup } => {
                backup::run_backup(&paths.switch_home, setup)?
            }
            cli::ProfileCommand::Restore { setup } => {
                backup::run_restore(&paths.switch_home, setup)?
            }
            cli::ProfileCommand::Import { path, cpa } => {
                let import_path = PathBuf::from(path);
                let fmt = if cpa {
                    profiles::ImportFormat::Cpa
                } else {
                    profiles::ImportFormat::Standard
                };
                profiles::import_profiles(
                    &paths.codex_home,
                    &paths.switch_home,
                    &import_path,
                    fmt,
                )?
                .render(format)?
            }
            cli::ProfileCommand::List => {
                profiles::list_profiles(&paths.codex_home, &paths.switch_home)?
                    .render(format)?
            }
            cli::ProfileCommand::Inspect { selector } => {
                let profiles =
                    profiles::list_profiles(&paths.codex_home, &paths.switch_home)?;
                let matches = matching_profiles_by_selector(&profiles, &selector);
                match matches.len() {
                    0 => {
                        // 尝试精确 id 匹配（selector 未匹配显示名/邮箱时兜底）
                        if let Some(profile) =
                            profiles.profiles.iter().find(|p| p.id == selector)
                        {
                            profile.render(format)?
                        } else {
                            anyhow::bail!("E_NOT_FOUND: 未找到 profile: {selector}")
                        }
                    }
                    1 => matches[0].render(format)?,
                    _ => {
                        // 多个匹配时返回数组（JSON）或文本列表
                        match format {
                            cli::OutputFormat::Json => {
                                serde_json::to_string_pretty(&matches)?
                            }
                            cli::OutputFormat::Text => matches
                                .iter()
                                .map(|p| {
                                    format!(
                                        "{} {} ({})",
                                        if p.active { "●" } else { "○" },
                                        p.name,
                                        p.id
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n"),
                        }
                    }
                }
            }
        },
        cli::Command::Completions { .. } => {
            unreachable!("completions handled before app paths")
        }
    };

    if !output.is_empty() {
        println!("{output}");
    }

    Ok(())
}

fn select_and_use_profile(
    codex_home: &std::path::Path,
    switch_home: &std::path::Path,
    candidates: model::ProfileListOutput,
) -> anyhow::Result<model::MutationResult> {
    if let Some(selected) = tui::select_profile(candidates)? {
        profiles::use_profile(codex_home, switch_home, &selected.id)
    } else {
        Ok(model::MutationResult {
            ok: false,
            action: "cancel".to_string(),
            id: None,
            name: None,
            email: None,
            ids: None,
            count: None,
            message: "已取消切换".to_string(),
        })
    }
}

fn resolve_app_paths() -> anyhow::Result<AppPaths> {
    let home = dirs::home_dir().context("无法获取用户 home 目录")?;

    Ok(AppPaths {
        codex_home: home.join(".codex"),
        switch_home: home.join(".codex-auth-switch"),
    })
}

fn matching_profiles_by_selector(
    profiles: &model::ProfileListOutput,
    selector: &str,
) -> Vec<model::ProfileSummary> {
    profiles
        .profiles
        .iter()
        .filter(|profile| {
            profile.name == selector
                || profile.email.as_deref() == Some(selector)
        })
        .cloned()
        .collect()
}

fn build_doctor_output(paths: &AppPaths) -> anyhow::Result<model::DoctorOutput> {
    let profiles_dir = paths.switch_home.join("profiles");
    let codex_home_exists = paths.codex_home.exists();
    let switch_home_exists = paths.switch_home.exists();
    let profiles_dir_exists = profiles_dir.exists();
    let profiles_count = if profiles_dir.exists() {
        fs::read_dir(&profiles_dir)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".json") && !name.starts_with('.'))
            })
            .count()
    } else {
        0
    };

    let state_path = paths.switch_home.join("state.json");
    let state_exists = state_path.exists();
    let state_json = if state_exists { fs::read(&state_path).ok() } else { None }
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let state_json_valid = if state_exists {
        state_json.is_some()
    } else {
        false
    };
    let active_profile = state_json.as_ref().and_then(|value| {
        value
            .get("active_profile")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
    });
    let active_profile_file_exists = active_profile
        .as_ref()
        .map(|id| profiles_dir.join(format!("{id}.json")).exists());

    let (webdav_configured, webdav_reachable, webdav_backups_count, webdav_error) =
        match backup::BackupConfig::load(&paths.switch_home)? {
            Some(config) => match backup::webdav_list_backups(&config) {
                Ok(backups) => (true, Some(true), Some(backups.len()), None),
                Err(error) => (true, Some(false), None, Some(error.to_string())),
            },
            None => (false, None, None, None),
        };

    Ok(model::DoctorOutput {
        codex_home: paths.codex_home.display().to_string(),
        switch_home: paths.switch_home.display().to_string(),
        codex_home_exists,
        switch_home_exists,
        profiles_dir_exists,
        auth_exists: paths.codex_home.join("auth.json").exists(),
        state_exists,
        state_json_valid,
        rollback_exists: paths.switch_home.join("rollback.json").exists(),
        profiles_count,
        webdav_configured,
        webdav_reachable,
        webdav_backups_count,
        webdav_error,
        active_profile,
        active_profile_file_exists,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        path::PathBuf,
        sync::{Mutex, MutexGuard, OnceLock},
    };

    use tempfile::tempdir;

    use crate::model::{ProfileListOutput, ProfileSummary};

    use super::{matching_profiles_by_selector, resolve_app_paths};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env() -> MutexGuard<'static, ()> {
        env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn set_env_var(key: &str, value: Option<OsString>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    struct EnvVarsGuard {
        home: Option<OsString>,
        codex_home: Option<OsString>,
    }

    impl EnvVarsGuard {
        fn capture() -> Self {
            Self {
                home: std::env::var_os("HOME"),
                codex_home: std::env::var_os("CODEX_HOME"),
            }
        }
    }

    impl Drop for EnvVarsGuard {
        fn drop(&mut self) {
            set_env_var("HOME", self.home.clone());
            set_env_var("CODEX_HOME", self.codex_home.clone());
        }
    }

    #[test]
    fn resolve_app_paths_uses_split_codex_directories() {
        let _guard = lock_env();
        let _env = EnvVarsGuard::capture();
        let home = tempdir().unwrap();

        set_env_var("HOME", Some(home.path().as_os_str().to_os_string()));
        set_env_var("CODEX_HOME", None);

        let resolved = resolve_app_paths().unwrap();

        assert_eq!(resolved.codex_home, PathBuf::from(home.path()).join(".codex"));
        assert_eq!(
            resolved.switch_home,
            PathBuf::from(home.path()).join(".codex-auth-switch")
        );
    }

    #[test]
    fn resolve_app_paths_ignore_codex_home_override() {
        let _guard = lock_env();
        let _env = EnvVarsGuard::capture();
        let home = tempdir().unwrap();

        set_env_var("HOME", Some(home.path().as_os_str().to_os_string()));
        set_env_var("CODEX_HOME", Some(OsString::from("/tmp/should-be-ignored")));

        let resolved = resolve_app_paths().unwrap();

        assert_eq!(resolved.codex_home, PathBuf::from(home.path()).join(".codex"));
        assert_eq!(
            resolved.switch_home,
            PathBuf::from(home.path()).join(".codex-auth-switch")
        );
    }

    #[test]
    fn matching_profiles_by_name_returns_all_duplicate_display_names() {
        let profiles = ProfileListOutput {
            active_profile: Some("ohanna27".to_string()),
            profiles: vec![
                ProfileSummary {
                    id: "ohanna27".to_string(),
                    name: "ohanna27".to_string(),
                    email: None,
                    subscription_plan: None,
                    account_id: None,
                    plan_type: None,
                    primary: None,
                    secondary: None,
                    active: true,
                },
                ProfileSummary {
                    id: "ohanna27-2".to_string(),
                    name: "ohanna27".to_string(),
                    email: None,
                    subscription_plan: None,
                    account_id: None,
                    plan_type: None,
                    primary: None,
                    secondary: None,
                    active: false,
                },
            ],
        };

        let matches = matching_profiles_by_selector(&profiles, "ohanna27");

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].id, "ohanna27");
        assert_eq!(matches[1].id, "ohanna27-2");
    }

    #[test]
    fn matching_profiles_by_selector_matches_email() {
        let profiles = ProfileListOutput {
            active_profile: None,
            profiles: vec![ProfileSummary {
                id: "alice1".to_string(),
                name: "alice".to_string(),
                email: Some("alice@example.com".to_string()),
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: false,
            }],
        };

        let matches = matching_profiles_by_selector(&profiles, "alice@example.com");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "alice1");
    }
}
