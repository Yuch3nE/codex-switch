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

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let codex_home = resolve_codex_home()?;

    let output = match cli.command {
        cli::Command::Account => auth::build_account_summary(&codex_home)?.render(cli.format)?,
        cli::Command::Usage => {
            let profiles = profiles::list_profiles(&codex_home)?;
            model::UsageTableOutput::from_profiles(profiles).render(cli.format)?
        }
        cli::Command::Profile { command } => match command {
            cli::ProfileCommand::Save { name } => {
                profiles::save_profile(&codex_home, name.as_deref())?
            }
            cli::ProfileCommand::Use { name } => match name {
                Some(name) => profiles::use_profile(&codex_home, &name)?,
                None => {
                    let profiles = profiles::list_profiles(&codex_home)?;
                    if profiles.profiles.is_empty() {
                        "暂无 profiles，可先使用 profile save 保存当前账号".to_string()
                    } else if let Some(selected) = tui::select_profile(profiles)? {
                        profiles::use_profile(&codex_home, &selected.id)?
                    } else {
                        "已取消切换".to_string()
                    }
                }
            },
            cli::ProfileCommand::Import { path } => {
                let import_path = PathBuf::from(path);
                profiles::import_profiles(&codex_home, &import_path)?
            }
            cli::ProfileCommand::List => {
                profiles::list_profiles(&codex_home)?.render(cli.format)?
            }
        },
    };

    if !output.is_empty() {
        println!("{output}");
    }

    Ok(())
}

fn resolve_codex_home() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(path));
    }

    let home =
        std::env::var_os("HOME").context("HOME is not set and CODEX_HOME was not provided")?;
    Ok(PathBuf::from(home).join(".codex"))
}
