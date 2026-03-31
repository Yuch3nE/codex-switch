# codex-switch

[English documentation](README.md) | [中文文档](README_CN.md)

[![Rust](https://img.shields.io/badge/Rust-1.73-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

## Quota Source Notes

- Account metadata is read from `~/.codex/auth.json`.
- Quota snapshots are read from the `rate_limits` field in `~/.codex/sessions/**/*.jsonl`.
- `primary` maps to the 5H limit and `secondary` maps to the weekly limit; free accounts usually only expose the weekly limit, and missing values are shown as `unknown`.

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
| 🩺 | `codex-switch doctor` | Diagnose local environment and profile state |
| 🏷️ | `codex-switch version` | Show version, git info, and build date |
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

Global flag for agents/CI:

- `--non-interactive`: disable TUI interactions and return coded errors when interactive selection would be required.

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

### doctor
Show local diagnostic status (paths/files/profile count):

```bash
codex-switch doctor
codex-switch --format json doctor
```

Health checks include:

- codex/switch directory existence
- profiles directory existence
- auth/state/rollback file presence
- state.json parse validity
- active profile file existence
- WebDAV configuration and connectivity (when backup config exists)

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

### profile inspect
Get detailed info for a single profile (by id, name, or email):

```bash
codex-switch profile inspect abc123
codex-switch --format json profile inspect alice@example.com
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

## Error Codes And Exit Codes

Exit codes:

| Code | Meaning |
|------|---------|
| `0` | Command completed successfully |
| `1` | General runtime error |
| `2` | Not found (`E_NOT_FOUND`) |
| `3` | Interactive required (`E_INTERACTIVE_REQUIRED`) |
| `4` | Ambiguous selector (`E_AMBIGUOUS_SELECTOR`) |

Machine-readable error prefixes in stderr:

- `E_NOT_FOUND`: the requested profile, file, or resource does not exist.
- `E_INTERACTIVE_REQUIRED`: command requires interactive selection, but `--non-interactive` is enabled.
- `E_AMBIGUOUS_SELECTOR`: selector matched multiple profiles and must be disambiguated by id.

### Mutation JSON Output

When `--format json` is set, `profile save`, `profile use`, `profile delete`, and `profile import` all return a structured JSON object instead of a plain string:

```json
{ "ok": true, "action": "use", "id": "abc123", "name": "work", "message": "已切换到 profile: work (id: abc123)" }
```

Cancelled TUI operations return `"ok": false` with `"action": "cancel"` (exit code 0).

Recommended automation pattern:

1. Run with `--non-interactive --format json` whenever possible.
2. If exit code `2` (`E_NOT_FOUND`), the profile or file does not exist — create it first.
3. If exit code `4` (`E_AMBIGUOUS_SELECTOR`), fetch candidates with `profile list --format json` and retry by `id`.
4. If exit code `3` (`E_INTERACTIVE_REQUIRED`), re-run with explicit selector (`id/name/email`) or `--auto`.

