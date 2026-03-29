# codex-switch

管理多个 Codex 账号，一键切换，随时查看所有账号额度。

## 功能

| 命令 | 功能 |
|------|------|
| `codex-switch account` | 查看当前登录账号信息 |
| `codex-switch usage` | 查看所有账号额度总览 |
| `codex-switch profile save` | 保存当前账号 |
| `codex-switch profile use` | 切换账号（TUI 交互或直接指定名称） |
| `codex-switch profile list` | 列出所有已保存账号 |
| `codex-switch profile delete` | 删除账号（TUI 多选） |
| `codex-switch profile import` | 导入 auth.json 文件 |
| `codex-switch profile backup` | 备份所有账号到 WebDAV |
| `codex-switch profile restore` | 从 WebDAV 恢复账号 |

## 安装

```bash
cargo install --path .
```

或直接编译：

```bash
cargo build --release
# 产物: target/release/codex-switch
```

## 快速开始

```bash
# 保存当前账号
codex-switch profile save

# 查看所有账号额度
codex-switch usage

# 交互式切换账号
codex-switch profile use
```

## 使用指南

### 查看当前账号

```bash
codex-switch account
```

显示当前 `~/.codex/auth.json` 对应的邮箱、订阅方案和 token 刷新时间。

### 查看所有账号额度

```bash
codex-switch usage
```

按订阅方案分组展示所有已保存账号的额度快照。当前激活账号会用 `●` 标记，并优先显示实时数据。

支持 JSON 格式输出：

```bash
codex-switch usage --format json
```

### 保存账号

```bash
# 自动以邮箱前缀命名
codex-switch profile save

# 自定义名称
codex-switch profile save work
```

### 切换账号

```bash
# TUI 交互选择
codex-switch profile use

# 直接指定名称
codex-switch profile use work
```

切换时会自动刷新当前账号的额度快照，再执行切换。

### 列出所有账号

```bash
codex-switch profile list
```

### 删除账号

```bash
codex-switch profile delete
```

进入 TUI 多选界面：`Space` 勾选，`Enter` 确认，`Esc`/`q` 退出。  
无法删除当前激活的账号，需先切换到其他账号。

### 导入账号

```bash
# 导入单个 auth.json（自动识别标准格式或 CPA 格式）
codex-switch profile import /path/to/auth.json

# 递归导入目录中所有 auth.json
codex-switch profile import /path/to/folder

# 导入 CPA 格式鉴权文件
codex-switch profile import --cpa /path/to/cpa.json

# 递归导入目录中所有 CPA .json 文件
codex-switch profile import --cpa /path/to/folder
```

导入只写入到 profiles，不切换当前激活账号。

### 备份到 WebDAV

```bash
# 有配置时直接备份；首次运行会弹出配置向导
codex-switch profile backup

# 修改 WebDAV 配置后再备份
codex-switch profile backup --setup
```

配置向导字段：

| 字段 | 说明 |
|------|------|
| WebDAV URL | 服务器地址，需以 `/` 结尾 |
| 用户名 / 密码 | WebDAV 鉴权凭据（密码以 `●` 掩码显示） |
| 远端目录 | 备份存放目录，默认 `codex-switch-backups/`（不含前导 `/`） |
| 最多备份数 | 服务器保留的最大备份数，`0` 不限制，默认 `10` |
| 加密口令 | 可选；填写后用 AES-256-GCM 加密，文件后缀改为 `.zip.enc` |

配置向导操作：`↑/↓` 切换字段，`Enter` 编辑，`Tab` 确认并跳下一项，`s` 保存，`Esc`/`q` 取消。

配置保存在 `~/.codex-auth-switch/backup.json`，后续直接复用。

### 从 WebDAV 恢复

```bash
# 有配置时直接列出备份；首次运行会弹出配置向导
codex-switch profile restore

# 修改配置后再恢复
codex-switch profile restore --setup
```

恢复流程：
1. 从服务器拉取备份列表（按时间倒序），TUI 选择版本
2. 解压后进入 TUI 多选：`Space` 勾选，`Enter` 确认，`Esc`/`q` 取消
3. 标有 `⚠` 的账号在本地已存在，导入时自动跳过（不覆盖）

如果备份已加密且配置无口令，会弹出口令输入框。

## 目录结构

```
~/.codex/                    # Codex 原始数据
    auth.json                # 当前激活账号凭据
    sessions/                # 实时额度数据

~/.codex-auth-switch/        # codex-switch 管理数据
    profiles/                # 已保存的所有账号
    state.json               # 当前激活账号记录
    rollback/                # 切换前的凭据备份
    backup.json              # WebDAV 备份配置
```

## 额度说明

`usage` 展示规则：
- **当前激活账号**：优先读取 `~/.codex/sessions` 实时数据，无则回退到快照
- **其他账号**：显示上次 `save` 或 `use` 时同步的快照

快照会在以下时机更新：
- `profile save` — 保存当前账号时
- `profile use` — 切换前刷新旧账号的快照

## 开发

```bash
cargo fmt
cargo test
cargo build --release
```

代码结构：

- `src/auth.rs` — 读取并解析鉴权文件
- `src/sessions.rs` — 扫描实时额度数据
- `src/profiles.rs` — profile 管理逻辑
- `src/backup.rs` — WebDAV 备份/恢复
- `src/tui.rs` — 全屏 TUI 交互组件
- `src/model.rs` — 输出格式（表格 / JSON）
- `src/cli.rs` — CLI 参数定义
- `src/main.rs` — 命令分发入口
- `tests/` — 集成测试（不依赖本机真实数据）
