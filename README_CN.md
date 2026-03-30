# codex-switch

[English documentation](README.md) | [中文文档](README_CN.md)

[![Rust](https://img.shields.io/badge/Rust-1.73-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

用于管理多个 Codex 账号，快速切换并追踪额度使用情况。

## 功能

- 将当前账号保存为可复用的 profile。
- 支持通过 profile id、名称或邮箱切换账号。
- 使用 `--auto` 自动选择周额度剩余最高的账号。
- 未指定选择器或命中多个候选时进入 TUI 交互选择。
- 支持 profile 导入，以及基于 WebDAV 的备份与恢复。
- 支持查看 5H 与周额度快照。

## 命令

| 图标 | 命令 | 说明 |
|------|------|------|
| 🧾 | `codex-switch account` | 查看当前账号信息 |
| 📊 | `codex-switch usage` | 查看所有 profile 额度总览 |
| 💾 | `codex-switch profile save [name]` | 将当前账号鉴权保存为 profile |
| 🔁 | `codex-switch profile use [name_or_email]` | 切换 profile（省略或模糊进入 TUI） |
| 🤖 | `codex-switch profile use --auto` | 自动选择周额度最优的 profile |
| 📋 | `codex-switch profile list` | 列出 profile |
| 🗑️ | `codex-switch profile delete [name_or_email]` | 通过选择器或 TUI 删除 profile |
| 📥 | `codex-switch profile import [path]` | 导入 auth.json / CPA 文件 |
| ☁️ | `codex-switch profile backup` | 备份到 WebDAV |
| 🔄 | `codex-switch profile restore` | 从 WebDAV 恢复 |
| 💡 | `codex-switch completions <shell>` | 生成补全脚本 |

适合 Agent/CI 的全局参数：

- `--non-interactive`：禁用 TUI 交互；若命令需要交互选择，会返回带错误码的报错信息。

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

- 当前活跃 profile 会记录到 `~/.codex-auth-switch/state.json`。
- `profile use` 会写入 `rollback.json` 以便安全回退。
- 支持使用 `--format json` 输出 JSON 格式结果。

