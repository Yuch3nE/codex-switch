use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "codex-switch")]
#[command(about = "管理多个 Codex 账号 profile，支持诊断、切换与额度总览")]
#[command(long_about = "管理多个 Codex 账号 profile 并查看额度总览。\n\n\
    支持保存、切换、删除、导入 Codex 鉴权文件，\n\
    支持 doctor 环境诊断，以及通过 WebDAV 备份/恢复所有 profiles。")]
pub struct Cli {
    #[arg(long, value_enum, default_value = "text", global = true, help = "输出格式")]
    pub format: OutputFormat,

    #[arg(long, global = true, help = "禁用 TUI 交互；若必须交互则直接报错")]
    pub non_interactive: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 显示当前激活账号的鉴权信息（邮箱、订阅方案、token 刷新时间）
    Account,
    /// 诊断本地环境状态（目录、配置文件、profiles、WebDAV 连通性）
    Doctor,
    /// 显示所有已保存 profile 的额度快照（当前激活账号优先用实时数据）
    Usage,
    /// 管理本地 Codex 账号 profiles
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// 输出 Shell 自动补全脚本（bash / zsh / fish / powershell）
    Completions {
        /// 目标 Shell
        shell: Shell,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// 将当前 ~/.codex/auth.json 保存为一个 profile（同时记录额度快照）
    Save {
        /// profile 显示名，省略时取邮箱 @ 前缀
        name: Option<String>,
    },
    /// 切换到指定 profile；可传 id/name/email，不传参数时进入 TUI
    Use {
        /// profile 选择器（id/显示名/邮箱）；存在多匹配时进入 TUI
        name: Option<String>,
        /// 自动选择周额度剩余最高的 profile 并切换（与 name 互斥）
        #[arg(long, short = 'a', conflicts_with = "name")]
        auto: bool,
    },
    /// 删除指定 profile；可传 id/name/email，省略时进入 TUI 多选
    Delete {
        /// 删除选择器（id/显示名/邮箱）；省略时进入 TUI 多选
        name: Option<String>,
    },
    /// 备份所有 profiles 到 WebDAV（有配置时直接执行，--setup 可重新配置）
    Backup {
        /// 打开 TUI 配置向导（重新配置 WebDAV 连接信息）
        #[arg(long)]
        setup: bool,
    },
    /// 从 WebDAV 备份文件恢复 profiles（有配置时直接列出，--setup 可重新配置）
    Restore {
        /// 打开 TUI 配置向导（重新配置 WebDAV 连接信息）
        #[arg(long)]
        setup: bool,
    },
    /// 导入 auth.json 文件或目录（不自动切换当前激活账号）
    Import {
        /// 以 CPA 鉴权格式（而非标准 auth.json）解析并导入
        #[arg(long)]
        cpa: bool,
        /// 要导入的文件路径或目录路径
        path: String,
    },
    /// 列出所有已保存的 profile（名称、id、是否激活）
    List,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, ProfileCommand};

    fn profile_command(cli: Cli) -> ProfileCommand {
        match cli.command {
            Command::Profile { command } => command,
            _ => panic!("expected profile command"),
        }
    }

    #[test]
    fn profile_use_allows_missing_name_for_interactive_mode() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "use"]).unwrap();
        match profile_command(cli) {
            ProfileCommand::Use { name, .. } => assert!(name.is_none()),
            _ => panic!("expected profile use command"),
        }
    }

    #[test]
    fn global_non_interactive_flag_parses() {
        let cli = Cli::try_parse_from([
            "codex-switch",
            "--non-interactive",
            "profile",
            "list",
        ])
        .unwrap();
        assert!(cli.non_interactive);
    }

    #[test]
    fn profile_import_accepts_cpa_flag() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "import", "--cpa", "sample.json"]).unwrap();
        match profile_command(cli) {
            ProfileCommand::Import { cpa, path } => {
                assert!(cpa);
                assert_eq!(path, "sample.json");
            }
            _ => panic!("expected profile import command"),
        }
    }

    #[test]
    fn profile_delete_parses_without_arguments() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "delete"]).unwrap();
        match profile_command(cli) {
            ProfileCommand::Delete { name } => assert!(name.is_none()),
            _ => panic!("expected profile delete command"),
        }
    }

    #[test]
    fn profile_delete_accepts_name_argument() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "delete", "work"]).unwrap();
        match profile_command(cli) {
            ProfileCommand::Delete { name } => assert_eq!(name.as_deref(), Some("work")),
            _ => panic!("expected profile delete command"),
        }
    }

    #[test]
    fn profile_backup_parses() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "backup"]).unwrap();
        match profile_command(cli) {
            ProfileCommand::Backup { setup } => assert!(!setup),
            _ => panic!("expected profile backup command"),
        }
    }

    #[test]
    fn profile_restore_parses() {
        let cli = Cli::try_parse_from(["codex-switch", "profile", "restore"]).unwrap();
        match profile_command(cli) {
            ProfileCommand::Restore { setup } => assert!(!setup),
            _ => panic!("expected profile restore command"),
        }
    }

    #[test]
    fn completions_parses_shell() {
        let cli = Cli::try_parse_from(["codex-switch", "completions", "zsh"]).unwrap();
        match cli.command {
            Command::Completions { shell } => assert_eq!(shell, clap_complete::Shell::Zsh),
            _ => panic!("expected completions command"),
        }
    }

    #[test]
    fn doctor_parses() {
        let cli = Cli::try_parse_from(["codex-switch", "doctor"]).unwrap();
        match cli.command {
            Command::Doctor => {}
            _ => panic!("expected doctor command"),
        }
    }
}
