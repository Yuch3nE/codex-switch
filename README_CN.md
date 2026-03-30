# codex-switch（中文文档）

[English documentation](README.md) | [中文文档](README_CN.md)

[![Rust](https://img.shields.io/badge/Rust-1.73-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/your-repo/codex-switch/actions/workflows/ci.yml/badge.svg)](https://github.com/your-repo/codex-switch/actions)

管理多个 Codex 账号，一键切换，查看额度总览。

## 功能

- 保存当前账号为 profile。
- 通过 id/name/email 切换 profile。
- `--auto` 自动选择周额度剩余最高的账号。
- 模糊时进入 TUI 选取。
- 支持 profile 导入、WebDAV 备份与恢复。
- 支持 5H / 周额度快照。

## 命令

| 图标 | 命令 | 说明 |
|------|------|------|
| 🧾 | `codex-switch account` | 查看当前账号信息 |
| 📊 | `codex-switch usage` | 查看所有 profile 额度总览 |
| 💾 | `codex-switch profile save [name]` | 保存当前账号为 profile |
| 🔁 | `codex-switch profile use [name_or_email]` | 切换 profile（省略或模糊进入 TUI） |
| 🤖 | `codex-switch profile use --auto` | 自动选择周额度最优 profile |
| 📋 | `codex-switch profile list` | 列出 profile |
| 🗑️ | `codex-switch profile delete [name_or_email]` | 删除 profile |
| 📥 | `codex-switch profile import [path]` | 导入 auth.json / CPA 文件 |
| ☁️ | `codex-switch profile backup` | 备份到 WebDAV |
| 🔄 | `codex-switch profile restore` | 从 WebDAV 恢复 |
| 💡 | `codex-switch completions <shell>` | 生成补全脚本 |

## 安装

```bash
cargo install --path .
```

或本地编译：

```bash
cargo build --release
```

## 快速开始

```bash
codex-switch profile save
codex-switch usage
codex-switch profile use
```

## 使用说明

### account

```bash
codex-switch account
```

### usage

```bash
codex-switch usage
```

### profile save

```bash
codex-switch profile save
codex-switch profile save work
```

### profile use

```bash
codex-switch profile use
codex-switch profile use work
codex-switch profile use alice@example.com
codex-switch profile use --auto
codex-switch profile use -a
```

### profile list

```bash
codex-switch profile list
```

### profile delete

```bash
codex-switch profile delete
codex-switch profile delete work
codex-switch profile delete alice@example.com
```

### profile import

```bash
codex-switch profile import /path/to/auth.json
codex-switch profile import --cpa /path/to/cpa.json
```

### profile backup / restore

```bash
codex-switch profile backup
codex-switch profile backup --setup
codex-switch profile restore
codex-switch profile restore --setup
```

## 说明

- 活跃 profile 记录在 `~/.codex-auth-switch/state.json`。
- `profile use` 有回滚机制（`rollback.json`）。
- 支持 `--format json` 输出。

