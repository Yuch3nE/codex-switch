# codex-switch

[English documentation](README.md) | [中文文档](README_CN.md)

[![Rust](https://img.shields.io/badge/Rust-1.73-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/your-repo/codex-switch/actions/workflows/ci.yml/badge.svg)](https://github.com/your-repo/codex-switch/actions)

A command-line tool for managing multiple Codex accounts, switching quickly, and tracking quota usage.

## Features

- Save the current account as a reusable profile.
- Switch by profile id, name, or email.
- Auto-switch to the profile with the highest weekly remaining quota (`--auto`).
- Use TUI selection when no selector is provided or multiple matches exist.
- Import profiles and perform WebDAV backup/restore.
- View quota snapshots for both 5H and weekly limits.

## Commands

| Icon | Command | Description |
|------|---------|-------------|
| 🧾 | `codex-switch account` | Show current account info |
| 📊 | `codex-switch usage` | Display quota overview for all saved profiles |
| 💾 | `codex-switch profile save [name]` | Save current account auth as a profile |
| 🔁 | `codex-switch profile use [name_or_email]` | Switch to profile (TUI when omitted or ambiguous) |
| 🤖 | `codex-switch profile use --auto` | Select and switch to the best weekly quota profile |
| 📋 | `codex-switch profile list` | List saved profiles |
| 🗑️ | `codex-switch profile delete [name_or_email]` | Delete profile(s) by selector or TUI |
| 📥 | `codex-switch profile import [path]` | Import auth.json or CPA JSON files |
| ☁️ | `codex-switch profile backup` | Backup profiles to WebDAV |
| 🔄 | `codex-switch profile restore` | Restore profiles from WebDAV |
| 💡 | `codex-switch completions <shell>` | Generate shell completions |

## Installation

```bash
cargo install --path .
```

or build locally:

```bash
cargo build --release
# executable at target/release/codex-switch
```

## Quick Start

```bash
codex-switch profile save
codex-switch usage
codex-switch profile use
```

## Usage

### account
Show current profile summary:

```bash
codex-switch account
```

### usage
Show snapshot quota table:

```bash
codex-switch usage
```

### profile save
Save current ~/.codex/auth.json as profile:

```bash
codex-switch profile save
codex-switch profile save work
```

### profile use
Switch by id/name/email or use interactive TUI:

```bash
codex-switch profile use
codex-switch profile use work
codex-switch profile use alice@example.com
codex-switch profile use --auto
codex-switch profile use -a
```

### profile list
List all profiles:

```bash
codex-switch profile list
```

### profile delete
Delete by selection or exact name/email:

```bash
codex-switch profile delete
codex-switch profile delete work
codex-switch profile delete alice@example.com
```

### profile import
Import auth files:

```bash
codex-switch profile import /path/to/auth.json
codex-switch profile import --cpa /path/to/cpa.json
```

### profile backup / restore
WebDAV backup:

```bash
codex-switch profile backup
codex-switch profile backup --setup
codex-switch profile restore
codex-switch profile restore --setup
```

## Notes

- Active profile state is tracked in ~/.codex-auth-switch/state.json.
- `profile use` writes a rollback copy for safer switching.
- JSON output is available with `--format json`.

