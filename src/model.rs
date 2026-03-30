use std::{
    cmp::Ordering,
    io::{stdout, IsTerminal},
};

use chrono::{Local, TimeZone};
use crossterm::terminal;
use serde::{Deserialize, Serialize};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::cli::OutputFormat;

const LOW_REMAINING_WARNING_THRESHOLD: f64 = 20.0;

#[derive(Debug, Clone, Serialize)]
pub struct AccountSummary {
    pub auth_mode: String,
    pub account_id: Option<String>,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub subscription_plan: Option<String>,
    pub last_refresh: Option<String>,
    pub organization_count: usize,
}

impl AccountSummary {
    pub fn render(&self, format: OutputFormat) -> anyhow::Result<String> {
        match format {
            OutputFormat::Json => Ok(serde_json::to_string_pretty(self)?),
            OutputFormat::Text => Ok(render_panel(
                "账号概览",
                &[
                    render_row("邮箱        ", self.email.as_deref().unwrap_or("")),
                    render_row(
                        "订阅等级    ",
                        self.subscription_plan.as_deref().unwrap_or(""),
                    ),
                    closing_row("最后刷新    ", self.last_refresh.as_deref().unwrap_or("")),
                ],
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorOutput {
    pub codex_home: String,
    pub switch_home: String,
    pub codex_home_exists: bool,
    pub switch_home_exists: bool,
    pub profiles_dir_exists: bool,
    pub auth_exists: bool,
    pub state_exists: bool,
    pub state_json_valid: bool,
    pub rollback_exists: bool,
    pub profiles_count: usize,
    pub webdav_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav_reachable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav_backups_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webdav_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile_file_exists: Option<bool>,
}

impl DoctorOutput {
    pub fn render(&self, format: OutputFormat) -> anyhow::Result<String> {
        match format {
            OutputFormat::Json => Ok(serde_json::to_string_pretty(self)?),
            OutputFormat::Text => {
                let overall_ok = self.auth_exists
                    && self.state_json_valid
                    && self
                        .active_profile_file_exists
                        .unwrap_or(true)
                    && self.webdav_reachable.unwrap_or(true);
                Ok(render_panel(
                    "环境诊断",
                    &[
                    render_row("总体健康   ", if overall_ok { "正常" } else { "需关注" }),
                    render_row("CODEX 目录   ", &self.codex_home),
                    render_row("SWITCH 目录  ", &self.switch_home),
                    render_row(
                        "codex 可访问 ",
                        if self.codex_home_exists { "是" } else { "否" },
                    ),
                    render_row(
                        "switch 可访问",
                        if self.switch_home_exists { "是" } else { "否" },
                    ),
                    render_row(
                        "profiles 目录",
                        if self.profiles_dir_exists { "存在" } else { "缺失" },
                    ),
                    render_row("auth.json   ", if self.auth_exists { "存在" } else { "缺失" }),
                    render_row("state.json  ", if self.state_exists { "存在" } else { "缺失" }),
                    render_row(
                        "state 有效性",
                        if self.state_json_valid { "有效" } else { "无效" },
                    ),
                    render_row(
                        "rollback.json",
                        if self.rollback_exists { "存在" } else { "缺失" },
                    ),
                    render_row("profiles 数量", &self.profiles_count.to_string()),
                    render_row(
                        "WebDAV 配置  ",
                        if self.webdav_configured { "已配置" } else { "未配置" },
                    ),
                    render_row(
                        "WebDAV 连通  ",
                        match self.webdav_reachable {
                            Some(true) => "正常",
                            Some(false) => "失败",
                            None => "未检查",
                        },
                    ),
                    render_row(
                        "远端备份数  ",
                        &self
                            .webdav_backups_count
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "未知".to_string()),
                    ),
                    render_row(
                        "WebDAV 错误 ",
                        self.webdav_error.as_deref().unwrap_or("无"),
                    ),
                    render_row(
                        "active 文件  ",
                        match self.active_profile_file_exists {
                            Some(true) => "存在",
                            Some(false) => "缺失",
                            None => "未设置",
                        },
                    ),
                    closing_row(
                        "当前 profile",
                        self.active_profile.as_deref().unwrap_or("未设置"),
                    ),
                    ],
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
}

impl TokenUsage {
    pub fn accumulate(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.reasoning_output_tokens += other.reasoning_output_tokens;
        self.total_tokens += other.total_tokens;
        let cached_total =
            self.cached_input_tokens.unwrap_or(0) + other.cached_input_tokens.unwrap_or(0);
        self.cached_input_tokens = if cached_total > 0
            || self.cached_input_tokens.is_some()
            || other.cached_input_tokens.is_some()
        {
            Some(cached_total)
        } else {
            None
        };
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    pub rollout_files: usize,
    pub rollout_files_with_token_count: usize,
    pub aggregate_tokens: TokenUsage,
    pub latest_session_file: Option<String>,
    pub latest_session_tokens: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<PrimaryRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<PrimaryRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
}

impl UsageSummary {
    pub fn empty() -> Self {
        Self {
            rollout_files: 0,
            rollout_files_with_token_count: 0,
            aggregate_tokens: TokenUsage::default(),
            latest_session_file: None,
            latest_session_tokens: None,
            primary: None,
            secondary: None,
            plan_type: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageTableOutput {
    pub profiles: Vec<UsageTableRow>,
}

impl UsageTableOutput {
    pub fn from_profiles(profiles: ProfileListOutput) -> Self {
        let mut rows = profiles
            .profiles
            .into_iter()
            .map(|profile| UsageTableRow {
                email: profile.email,
                subscription_plan: profile.subscription_plan,
                plan_type: profile.plan_type,
                primary: profile.primary,
                secondary: profile.secondary,
                active: profile.active,
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            usage_plan_rank(effective_plan(left.plan_type.as_deref(), left.subscription_plan.as_deref()))
                .cmp(&usage_plan_rank(effective_plan(right.plan_type.as_deref(), right.subscription_plan.as_deref())))
                .then_with(|| {
                    usage_sort_remaining(right)
                        .partial_cmp(&usage_sort_remaining(left))
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| left.email.cmp(&right.email))
        });

        Self { profiles: rows }
    }

    pub fn render(&self, format: OutputFormat) -> anyhow::Result<String> {
        match format {
            OutputFormat::Json => Ok(serde_json::to_string_pretty(self)?),
            OutputFormat::Text => Ok(self.render_table()),
        }
    }

    fn render_table(&self) -> String {
        if self.profiles.is_empty() {
            return "暂无已保存账号，可先执行 profile save".to_string();
        }

        let headers = ["邮箱", "订阅方案", "5H额度", "5H重置", "周额度", "周重置"];
        let mut rows: Vec<(String, [String; 6])> = self
            .profiles
            .iter()
            .map(|profile| {
                let plan = effective_plan(
                    profile.plan_type.as_deref(),
                    profile.subscription_plan.as_deref(),
                );
                let five_hour = render_five_hour_limit(plan, profile.primary.as_ref());
                let weekly = render_weekly_limit(plan, profile.primary.as_ref(), profile.secondary.as_ref());
                (
                    usage_plan_label(plan),
                    [
                        render_active_email(profile.active, profile.email.as_deref()),
                        usage_plan_label(plan),
                        five_hour.0,
                        five_hour.1,
                        weekly.0,
                        weekly.1,
                    ],
                )
            })
            .collect();

        adapt_grouped_email_column(&mut rows);

        let mut widths = [0usize; 6];
        for (index, header) in headers.iter().enumerate() {
            widths[index] = display_width(header);
        }
        for (_, row) in &rows {
            for (index, value) in row.iter().enumerate() {
                widths[index] = widths[index].max(display_width(value));
            }
        }

        let mut lines = Vec::new();
        lines.push(colorize_header("╭─ 账号额度总览"));
        let header_row = headers.map(ToString::to_string);

        for (index, (group, row)) in rows.iter().enumerate() {
            let starts_group = index == 0 || rows[index - 1].0 != *group;
            let ends_group = index + 1 == rows.len() || rows[index + 1].0 != *group;

            if starts_group {
                lines.push(colorize_group_heading(&format!("├─ {}", group)));
                lines.push(render_emphasized_table_rule('╞', '╪', '╡', &widths));
                lines.push(render_table_row(&header_row, &widths, true));
                lines.push(render_table_rule('├', '┼', '┤', &widths));
            }

            lines.push(render_table_row(row, &widths, false));

            if ends_group {
                lines.push(render_emphasized_table_rule('╘', '╧', '╛', &widths));
            } else {
                lines.push(render_table_rule('├', '┼', '┤', &widths));
            }
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageTableRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<PrimaryRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<PrimaryRateLimit>,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrimaryRateLimit {
    pub used_percent: f64,
    pub window_minutes: u64,
    pub resets_at: u64,
}

impl PrimaryRateLimit {
    pub fn render_remaining_progress(&self) -> String {
        let width = 20usize;
        let remaining_percent = (100.0 - self.used_percent).clamp(0.0, 100.0);
        let warning = remaining_percent < LOW_REMAINING_WARNING_THRESHOLD;
        let filled = ((remaining_percent / 100.0) * width as f64)
            .floor()
            .clamp(0.0, width as f64) as usize;
        let filled_bar = "█".repeat(filled);
        let empty_bar = "░".repeat(width.saturating_sub(filled));
        let filled_rendered = if warning {
            colorize_progress_warning(&filled_bar)
        } else {
            colorize_progress_filled(&filled_bar)
        };
        let empty_rendered = if warning {
            colorize_progress_warning(&empty_bar)
        } else {
            colorize_progress_empty(&empty_bar)
        };
        let warning_suffix = if warning {
            format!(" {}", colorize_progress_warning("⚠"))
        } else {
            String::new()
        };
        format!(
            "╢{}{}╟ {:.1}%{}",
            filled_rendered,
            empty_rendered,
            remaining_percent,
            warning_suffix,
        )
    }

    pub fn render_reset_time(&self) -> String {
        match Local.timestamp_opt(self.resets_at as i64, 0).single() {
            Some(value) => value.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => self.resets_at.to_string(),
        }
    }
}

fn render_panel(title: &str, rows: &[String]) -> String {
    if rows.is_empty() {
        return format!("╭─ {}\n╰─", title);
    }

    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(colorize_header(&format!("╭─ {}", title)));
    lines.extend(rows.iter().cloned());
    lines.join("\n")
}

fn render_table_rule(left: char, middle: char, right: char, widths: &[usize; 6]) -> String {
    render_table_rule_with_fill(left, middle, right, widths, '─')
}

fn render_emphasized_table_rule(
    left: char,
    middle: char,
    right: char,
    widths: &[usize; 6],
) -> String {
    render_table_rule_with_fill(left, middle, right, widths, '═')
}

fn render_table_rule_with_fill(
    left: char,
    middle: char,
    right: char,
    widths: &[usize; 6],
    fill: char,
) -> String {
    let separator = middle.to_string();
    let segments = widths
        .iter()
        .map(|width| std::iter::repeat(fill).take(*width + 2).collect::<String>())
        .collect::<Vec<_>>();
    colorize_border(&format!("{}{}{}", left, segments.join(&separator), right))
}

fn render_table_row(values: &[String; 6], widths: &[usize; 6], is_header: bool) -> String {
    let separator = colorize_border("│");
    let rendered = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let padding = widths[index].saturating_sub(display_width(value));
            let content = format!(" {}{} ", value, " ".repeat(padding));
            if is_header {
                colorize_table_header(&content)
            } else {
                content
            }
        })
        .collect::<Vec<_>>()
        .join(&separator);

    format!("{}{}{}", separator, rendered, colorize_border("│"))
}

fn adapt_grouped_email_column(rows: &mut [(String, [String; 6])]) {
    let Some(terminal_width) = terminal_width() else {
        return;
    };

    let reserved = 64usize;
    if terminal_width <= reserved {
        return;
    }

    let max_email_width = terminal_width.saturating_sub(reserved).clamp(18, 42);
    for (_, row) in rows {
        row[0] = truncate_for_width(&row[0], max_email_width);
    }
}

fn terminal_width() -> Option<usize> {
    if !supports_color() {
        return None;
    }

    terminal::size().ok().map(|(width, _)| width as usize)
}

fn truncate_for_width(value: &str, max_width: usize) -> String {
    if display_width(value) <= max_width {
        return value.to_string();
    }

    let mut width = 0usize;
    let mut truncated = String::new();

    for ch in value.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + char_width + 1 > max_width {
            break;
        }
        truncated.push(ch);
        width += char_width;
    }

    format!("{}…", truncated)
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(strip_ansi(value).as_str())
}

fn strip_ansi(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next == 'm' {
                    break;
                }
            }
            continue;
        }

        result.push(ch);
    }

    result
}

fn render_row(label: &str, value: &str) -> String {
    format!("│ {}│ {}", colorize_label(label), value)
}

fn closing_row(label: &str, value: &str) -> String {
    format!("╰─ {}│ {}", colorize_label(label), value)
}

fn colorize(text: &str, ansi_code: &str) -> String {
    if supports_color() {
        format!("\x1b[{}m{}\x1b[0m", ansi_code, text)
    } else {
        text.to_string()
    }
}

fn colorize_header(text: &str) -> String {
    colorize(text, "1;38;5;51")
}

fn colorize_label(text: &str) -> String {
    colorize(text, "33")
}

fn colorize_progress_filled(text: &str) -> String {
    colorize(text, "1;38;5;46")
}

fn colorize_progress_empty(text: &str) -> String {
    colorize(text, "38;5;240")
}

fn colorize_progress_warning(text: &str) -> String {
    colorize(text, "1;38;5;196")
}

fn colorize_border(text: &str) -> String {
    colorize(text, "38;5;39")
}

fn colorize_table_header(text: &str) -> String {
    colorize(text, "1;38;5;228")
}

fn colorize_group_heading(text: &str) -> String {
    colorize(text, "1;38;5;219")
}

fn render_active_email(active: bool, email: Option<&str>) -> String {
    let email = email.unwrap_or_default();
    if !active {
        return email.to_string();
    }

    if supports_color() {
        format!("\x1b[1;38;5;46m●\x1b[0m {}", email)
    } else {
        format!("● {}", email)
    }
}

fn effective_plan<'a>(
    plan_type: Option<&'a str>,
    subscription_plan: Option<&'a str>,
) -> Option<&'a str> {
    plan_type.or(subscription_plan)
}

fn usage_sort_remaining(row: &UsageTableRow) -> f64 {
    weekly_limit_for_plan(
        effective_plan(row.plan_type.as_deref(), row.subscription_plan.as_deref()),
        row.primary.as_ref(),
        row.secondary.as_ref(),
    )
    .or(row.primary.as_ref())
    .map(|value| 100.0 - value.used_percent)
    .unwrap_or(0.0)
}

fn profile_weekly_remaining(profile: &ProfileSummary) -> f64 {
    weekly_limit_for_plan(
        effective_plan(
            profile.plan_type.as_deref(),
            profile.subscription_plan.as_deref(),
        ),
        profile.primary.as_ref(),
        profile.secondary.as_ref(),
    )
    .or(profile.primary.as_ref())
    .map(|value| 100.0 - value.used_percent)
    .unwrap_or(0.0)
}

fn weekly_limit_for_plan<'a>(
    plan: Option<&str>,
    primary: Option<&'a PrimaryRateLimit>,
    secondary: Option<&'a PrimaryRateLimit>,
) -> Option<&'a PrimaryRateLimit> {
    match plan.unwrap_or_default().to_ascii_lowercase().as_str() {
        "free" => secondary.or(primary),
        _ => secondary,
    }
}

fn unknown_limit_pair() -> (String, String) {
    ("未知".to_string(), "未知".to_string())
}

fn render_five_hour_limit(
    plan: Option<&str>,
    primary: Option<&PrimaryRateLimit>,
) -> (String, String) {
    if matches!(plan.unwrap_or_default().to_ascii_lowercase().as_str(), "free") {
        return unknown_limit_pair();
    }

    match primary {
        Some(limit) => (limit.render_remaining_progress(), limit.render_reset_time()),
        None => unknown_limit_pair(),
    }
}

fn render_weekly_limit(
    plan: Option<&str>,
    primary: Option<&PrimaryRateLimit>,
    secondary: Option<&PrimaryRateLimit>,
) -> (String, String) {
    match weekly_limit_for_plan(plan, primary, secondary) {
        Some(limit) => (limit.render_remaining_progress(), limit.render_reset_time()),
        None => unknown_limit_pair(),
    }
}

fn usage_plan_rank(plan: Option<&str>) -> usize {
    match plan.unwrap_or_default().to_ascii_lowercase().as_str() {
        "pro" => 0,
        "plus" => 1,
        "team" => 2,
        "free" => 3,
        _ => 4,
    }
}

fn usage_plan_label(plan: Option<&str>) -> String {
    let normalized = plan.unwrap_or("unknown").trim();
    if normalized.is_empty() {
        "UNKNOWN".to_string()
    } else {
        normalized.to_ascii_uppercase()
    }
}

fn supports_color() -> bool {
    stdout().is_terminal()
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub subscription_plan: Option<String>,
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<PrimaryRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<PrimaryRateLimit>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileListOutput {
    pub active_profile: Option<String>,
    pub profiles: Vec<ProfileSummary>,
}

impl ProfileListOutput {
    /// 从所有 profiles 中选出周额度剩余最高的 profile。
    /// 排序规则：plan 等级升序（pro > plus > team > free > 其他），再按周额度剩余比例降序。
    pub fn best_profile(&self) -> Option<&ProfileSummary> {
        self.profiles.iter().max_by(|left, right| {
            let rank_l = usage_plan_rank(effective_plan(
                left.plan_type.as_deref(),
                left.subscription_plan.as_deref(),
            ));
            let rank_r = usage_plan_rank(effective_plan(
                right.plan_type.as_deref(),
                right.subscription_plan.as_deref(),
            ));
            // rank 越小等级越高，所以用 rank_r.cmp(rank_l) 让等级高的排前面
            rank_r.cmp(&rank_l).then_with(|| {
                profile_weekly_remaining(left)
                    .partial_cmp(&profile_weekly_remaining(right))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
    }

    pub fn render(&self, format: OutputFormat) -> anyhow::Result<String> {
        match format {
            OutputFormat::Json => Ok(serde_json::to_string_pretty(self)?),
            OutputFormat::Text => {
                let mut lines = vec![
                    "╭─ Profiles".to_string(),
                    format!(
                        "│ 当前 profile: {}",
                        self.active_profile.as_deref().unwrap_or("")
                    ),
                ];

                for profile in &self.profiles {
                    lines.push(format!(
                        "├─ {} {} (id: {})",
                        if profile.active { "●" } else { "○" },
                        profile.name,
                        profile.id,
                    ));
                    lines.push(format!(
                        "│  邮箱: {}",
                        profile.email.as_deref().unwrap_or(""),
                    ));
                    lines.push(format!(
                        "│  订阅等级: {}{}",
                        profile.subscription_plan.as_deref().unwrap_or(""),
                        if profile.active { " | 当前" } else { "" }
                    ));
                }

                if self.profiles.is_empty() {
                    lines.push("╰─ 暂无 profiles".to_string());
                } else {
                    lines.push("╰─ 结束".to_string());
                }

                Ok(lines.join("\n"))
            }
        }
    }
}
