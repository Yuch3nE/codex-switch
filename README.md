# codex-switch

一个用 Rust 写的 Codex 账号切换与额度查看工具。

当前版本聚焦两件事：

- 管理多个 Codex 账号 profile
- 以静态 TUI 风格查看多账号额度总览

## 当前能力

- `account`
  - 读取 `~/.codex/auth.json`
  - 解析 JWT，显示当前账号邮箱、订阅信息和最后刷新时间
- `usage`
  - 读取已保存的 profile
  - 按订阅方案分组展示账号额度表
  - 组内按剩余额度从高到低排序
  - 当前激活账号会额外高亮
- `profile save [name]`
  - 把当前 `~/.codex/auth.json` 纳入管理
  - 同时保存当前账号的额度快照
  - `name` 可省略，默认取邮箱 `@` 前面的部分
- `profile use [name|id]`
  - 切换到指定 profile
  - 如果不传参数，会进入交互式 TUI 选择器
  - 切换前会先刷新当前激活账号的额度快照
- `profile import <path>`
  - 导入指定 `auth.json` 文件
  - 或递归导入某个目录树下的所有 `auth.json`
  - 只导入到 profiles，不会自动切换当前激活账号
- `profile list`
  - 查看已保存 profile 列表

## 安装与运行

```bash
cargo build
cargo run -- --help
cargo run -- usage
cargo run -- profile list
```

安装到本机：

```bash
cargo install --path .
codex-switch --help
```

如果你不想直接读取默认目录，可以通过 `CODEX_HOME` 指向自定义 Codex 数据目录：

```bash
CODEX_HOME=/path/to/.codex cargo run -- usage
```

如果你想在仓库内构造一套测试数据，建议把它放在项目根目录下的 `.codex/`，该目录已经在 `.gitignore` 中默认忽略：

```bash
CODEX_HOME=$PWD/.codex cargo run -- usage
```

## 常用工作流

### 1. 保存当前账号

```bash
cargo run -- profile save
```

或手动指定名字：

```bash
cargo run -- profile save work
```

### 2. 列出已保存账号

```bash
cargo run -- profile list
```

### 3. 导入外部 auth.json

导入单个文件：

```bash
cargo run -- profile import /path/to/auth.json
```

递归导入目录中的所有 `auth.json`：

```bash
cargo run -- profile import /path/to/folder
```

导入行为说明：

- 如果传入的是文件，就按单文件导入
- 如果传入的是目录，就递归扫描目录树中的所有 `auth.json`
- 导入不会自动切换当前激活 profile
- 导入得到的 profile 默认使用邮箱前缀作为显示名，内部 id 会自动去重

### 4. 切换账号

直接指定：

```bash
cargo run -- profile use work
```

进入交互式选择器：

```bash
cargo run -- profile use
```

### 5. 查看多账号额度

```bash
cargo run -- usage
```

文本输出会按订阅方案分组，例如 `PRO`、`PLUS`、`TEAM`、`FREE`。

JSON 输出：

```bash
cargo run -- usage --format json
```

## 额度刷新逻辑

这是当前版本最重要的行为约束。

### 实时额度来源

`~/.codex/sessions` 中的额度信息，只能代表当前 `~/.codex/auth.json` 对应的那个账号。

换句话说：

- `sessions` 不是一个“所有账号共享的额度仓库”
- 它只能作为“当前激活账号”的实时额度源

### 快照写入规则

- `profile save` 时，会把当前账号的实时额度一起写进该 profile 的快照
- `profile import` 时，只导入认证信息，不会伪造额度快照
- `profile use` 时，会在切走前先把当前激活账号的额度快照刷新一次

### usage 展示规则

- 当前激活账号：优先显示当前 `sessions` 的实时额度
- 如果当前激活账号没有实时会话数据：回退到该 profile 上次保存的额度快照
- 非激活账号：只显示各自 profile 内最后一次同步的额度快照

### “最新额度”的判定方式

读取 `~/.codex/sessions` 时，按以下规则确定最新额度：

1. 找到最新日期目录
2. 在该目录里找到最新的 `rollout-*.jsonl`
3. 读取该文件最后一条 `token_count`
4. 取其中的 `rate_limits.primary`

## 当前限制

- 其他非激活 profile 无法实时刷新额度，只能显示上次同步的快照
- 如果某个账号从未保存过，`usage` 不会显示它
- 当前激活账号的实时额度依赖 `~/.codex/sessions` 中确实存在对应 rollout 数据
- `summary` 命令已移除，统一由 `usage` 承担额度总览能力

## 输出说明

`usage` 表格默认包含这些列：

- 邮箱
- 订阅方案
- 剩余额度
- 窗口周期
- 重置时间

当前激活账号会用 `●` 标记。

## 开发

常用命令：

```bash
cargo fmt
cargo test
```

当前代码结构：

- `src/auth.rs`: 读取 `auth.json` 并解析 JWT
- `src/sessions.rs`: 扫描 `rollout-*.jsonl` 并汇总额度
- `src/profiles.rs`: 管理 profile 的保存、切换、导入与快照
- `src/model.rs`: 文本表格和 JSON 输出模型
- `src/tui.rs`: `profile use` 的交互式全屏选择器
- `src/main.rs`: CLI 命令分发入口

测试位于 `tests/` 目录，均使用临时目录构造 `.codex` 数据，不依赖你机器上的真实账号环境。

## 测试

```bash
cargo test
```