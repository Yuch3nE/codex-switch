use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "codex-switch")]
#[command(about = "Inspect Codex account and local usage data")]
pub struct Cli {
    #[arg(long, value_enum, default_value = "text", global = true)]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Account,
    Usage,
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    Save { name: Option<String> },
    Use { name: Option<String> },
    Delete,
    /// 备份所有 profiles 到 WebDAV。首次需配置连接信息，后续直接备份。
    /// 如需修改配置请加 --setup。
    Backup {
        /// 打开 TUI 配置向导（重新配置 WebDAV 连接信息）
        #[arg(long)]
        setup: bool,
    },
    /// 从 WebDAV 备份文件恢复 profiles。首次需配置连接信息。
    /// 如需修改配置请加 --setup。
    Restore {
        /// 打开 TUI 配置向导（重新配置 WebDAV 连接信息）
        #[arg(long)]
        setup: bool,
    },
    Import { #[arg(long)] cpa: bool, path: String },
    List,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, ProfileCommand};

    #[test]
    fn profile_use_allows_missing_name_for_interactive_mode() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "use"]).unwrap();

        match cli.command {
            Command::Profile { command } => match command {
                ProfileCommand::Use { name } => assert!(name.is_none()),
                _ => panic!("expected profile use command"),
            },
            _ => panic!("expected profile command"),
        }
    }

    #[test]
    fn profile_import_accepts_cpa_flag() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "import", "--cpa", "sample.json"]).unwrap();

        match cli.command {
            Command::Profile { command } => match command {
                ProfileCommand::Import { cpa, path } => {
                    assert!(cpa);
                    assert_eq!(path, "sample.json");
                }
                _ => panic!("expected profile import command"),
            },
            _ => panic!("expected profile command"),
        }
    }

    #[test]
    fn profile_delete_parses_without_arguments() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "delete"]).unwrap();

        match cli.command {
            Command::Profile { command } => match command {
                ProfileCommand::Delete => {}
                _ => panic!("expected profile delete command"),
            },
            _ => panic!("expected profile command"),
        }
    }

    #[test]
    fn profile_backup_parses() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "backup"]).unwrap();

        match cli.command {
            Command::Profile { command } => match command {
                ProfileCommand::Backup { setup } => assert!(!setup),
                _ => panic!("expected profile backup command"),
            },
            _ => panic!("expected profile command"),
        }
    }

    #[test]
    fn profile_restore_parses() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "restore"]).unwrap();

        match cli.command {
            Command::Profile { command } => match command {
                ProfileCommand::Restore { setup } => assert!(!setup),
                _ => panic!("expected profile restore command"),
            },
            _ => panic!("expected profile command"),
        }
    }
}
