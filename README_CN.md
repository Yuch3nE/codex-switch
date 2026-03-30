# codex-switch（中文文档）

[English documentation](README.md) | [中文文档](README_CN.md)

[![Rust](https://img.shields.io/badge/Rust-1.73-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/your-repo/codex-switch/actions/workflows/ci.yml/badge.svg)](https://github.com/your-repo/codex-switch/actions)

管理多个 Codex 账号，一键切换，随时查看所有账号额度。

## 功能 / Features

| 图标 | 命令 / Command | 描述 / Description |
|------|----------------|-------------------|
| 🧾 | `codex-switch account` | 查看当前登录账号信息 / Show current account details |
| 📊 | `codex-switch usage` | 查看所有账号额度总览 / View quota summary for all accounts |
| 💾 | `codex-switch profile save` | 保存当前账号 / Save current account as profile |
| 🔁 | `codex-switch profile use` | 切换账号（TUI 交互或直接指定名称/邮箱） / Switch account (TUI or by name/email) |
| 🤖 | `codex-switch profile use --auto` | 自动切换到周额度剩余最高的账号 / Auto-switch to highest weekly remaining quota profile |
| 📋 | `codex-switch profile list` | 列出所有已保存账号 / List all saved profiles |
| 🗑️ | `codex-switch profile delete` | 删除账号（TUI 多选，或直接指定名称/邮箱） / Delete profile(s) (TUI or by name/email) |
| 📥 | `codex-switch profile import` | 导入 auth.json 文件 / Import auth.json files |
| ☁️ | `codex-switch profile backup` | 备份所有账号到 WebDAV / Backup profiles to WebDAV |
| 🔄 | `codex-switch profile restore` | 从 WebDAV 恢复账号 / Restore profiles from WebDAV |
| 💡 | `codex-switch completions` | 生成 Shell 自动补全脚本 / Generate shell completions |

## 安装

```bash
cargo install --path .
```

或直接编译：

```bash
cargo build --release
```

## 快速开始

```bash
codex-switch profile save
codex-switch usage
codex-switch profile use
```

## 详细用法

更多使用说明请参考 `README.md`（英文）。
