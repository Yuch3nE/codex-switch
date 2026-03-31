# codex-switch

[English documentation](README.md) | [中文文档](README_CN.md)

[![Rust](https://img.shields.io/badge/Rust-1.73-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

## 额度来源说明

- 账号基础信息读取自 `~/.codex/auth.json`。
- 额度快照读取自 `~/.codex/sessions/**/*.jsonl` 中的 `rate_limits` 字段。
- 其中 `primary` 对应 5H 限额，`secondary` 对应周限额；free 账号通常只提供周限额，缺失值会显示为“未知”。

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
| 🩺 | `codex-switch doctor` | 诊断本地环境与 profile 状态 |
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

### doctor

```bash
codex-switch doctor
codex-switch --format json doctor
```

健康检查项包括：

- codex/switch 目录是否存在
- profiles 目录是否存在
- auth/state/rollback 文件是否存在
- state.json 是否可解析
- active profile 对应文件是否存在
- WebDAV 是否已配置，以及在有配置时的连通性

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

### profile inspect
查询单个 profile 详情（支持 id/name/email）：

```bash
codex-switch profile inspect abc123
codex-switch --format json profile inspect alice@example.com
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

## 错误码与退出码

退出码约定：

| 码 | 含义 |
|------|---------|
| `0` | 命令成功 |
| `1` | 运行时错误 |
| `2` | 资源不存在（`E_NOT_FOUND`） |
| `3` | 需要交互（`E_INTERACTIVE_REQUIRED`） |
| `4` | 选择器歧义（`E_AMBIGUOUS_SELECTOR`） |

stderr 中的机器可识别错误前缀：

- `E_NOT_FOUND`：请求的 profile、文件或资源不存在。
- `E_INTERACTIVE_REQUIRED`：命令需要交互选择，但启用了 `--non-interactive`。
- `E_AMBIGUOUS_SELECTOR`：选择器匹配到多个 profile，需要改用 id 明确指定。

### Mutation 命令的 JSON 输出

当使用 `--format json` 时，`profile save`、`profile use`、`profile delete`、`profile import` 均返回结构化 JSON 对象：

```json
{ "ok": true, "action": "use", "id": "abc123", "name": "work", "message": "已切换到 profile: work (id: abc123)" }
```

TUI 取消操作返回 `"ok": false`、`"action": "cancel"`（退出码为 0）。

推荐自动化处理方式：

1. 尽量使用 `--non-interactive --format json`。
2. 若退出码为 `2`（`E_NOT_FOUND`），profile 或文件不存在，先创建。
3. 若退出码为 `4`（`E_AMBIGUOUS_SELECTOR`），用 `profile list --format json` 获取候选，再用 id 重试。
4. 若退出码为 `3`（`E_INTERACTIVE_REQUIRED`），改为显式传入 `id/name/email` 或使用 `--auto`。

