mod auth;
mod cli;
mod jwt;
mod model;
mod profiles;
mod sessions;
mod tui;

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

struct AppPaths {
    codex_home: PathBuf,
    switch_home: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let paths = resolve_app_paths()?;

    let output = match cli.command {
        cli::Command::Account => auth::build_account_summary(&paths.codex_home)?.render(cli.format)?,
        cli::Command::Usage => {
            let profiles = profiles::list_profiles(&paths.codex_home, &paths.switch_home)?;
            model::UsageTableOutput::from_profiles(profiles).render(cli.format)?
        }
        cli::Command::Profile { command } => match command {
            cli::ProfileCommand::Save { name } => {
                profiles::save_profile(&paths.codex_home, &paths.switch_home, name.as_deref())?
            }
            cli::ProfileCommand::Use { name } => match name {
                Some(name) => profiles::use_profile(&paths.codex_home, &paths.switch_home, &name)?,
                None => {
                    let profiles = profiles::list_profiles(&paths.codex_home, &paths.switch_home)?;
                    if profiles.profiles.is_empty() {
                        "暂无 profiles，可先使用 profile save 保存当前账号".to_string()
                    } else if let Some(selected) = tui::select_profile(profiles)? {
                        profiles::use_profile(&paths.codex_home, &paths.switch_home, &selected.id)?
                    } else {
                        "已取消切换".to_string()
                    }
                }
            },
            cli::ProfileCommand::Import { path } => {
                let import_path = PathBuf::from(path);
                profiles::import_profiles(&paths.codex_home, &paths.switch_home, &import_path)?
            }
            cli::ProfileCommand::List => {
                profiles::list_profiles(&paths.codex_home, &paths.switch_home)?.render(cli.format)?
            }
        },
    };

    if !output.is_empty() {
        println!("{output}");
    }

    Ok(())
}

fn resolve_app_paths() -> anyhow::Result<AppPaths> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    let home = PathBuf::from(home);

    Ok(AppPaths {
        codex_home: home.join(".codex"),
        switch_home: home.join(".codex-auth-switch"),
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

    use super::resolve_app_paths;

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
}
