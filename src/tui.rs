use std::{
    collections::BTreeSet,
    io::{self, IsTerminal},
    time::Duration,
};

use anyhow::Context;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};

use crate::model::{ProfileListOutput, ProfileSummary};

pub fn select_profile(output: ProfileListOutput) -> anyhow::Result<Option<ProfileSummary>> {
    if output.profiles.is_empty() {
        return Ok(None);
    }

    if !io::stdout().is_terminal() {
        anyhow::bail!("交互式 TUI 需要在真实终端中运行");
    }

    let mut selector = ProfileSelectorState::new(output.profiles);
    with_tui(|terminal| run_selector(terminal, &mut selector))
}

pub fn select_profiles_to_delete(
    output: ProfileListOutput,
) -> anyhow::Result<Option<Vec<ProfileSummary>>> {
    if output.profiles.is_empty() {
        return Ok(None);
    }

    if !io::stdout().is_terminal() {
        anyhow::bail!("交互式 TUI 需要在真实终端中运行");
    }

    let mut selector = DeleteSelectionState::new(output.profiles);
    with_tui(|terminal| run_delete_selector(terminal, &mut selector))
}

pub fn select_backup_profiles(
    profiles: Vec<ProfileSummary>,
    existing_ids: std::collections::HashSet<String>,
) -> anyhow::Result<Option<Vec<ProfileSummary>>> {
    if profiles.is_empty() {
        return Ok(Some(vec![]));
    }

    if !io::stdout().is_terminal() {
        anyhow::bail!("交互式 TUI 需要在真实终端中运行");
    }

    let mut selector = BackupSelectionState::new(profiles, existing_ids);
    with_tui(|terminal| run_backup_selector(terminal, &mut selector))
}

fn with_tui<F, T>(f: F) -> anyhow::Result<T>
where
    F: FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<T>,
{
    let mut stdout = io::stdout();
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal backend")?;

    let result = f(&mut terminal);

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

fn run_backup_selector(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    selector: &mut BackupSelectionState,
) -> anyhow::Result<Option<Vec<ProfileSummary>>> {
    loop {
        terminal.draw(|frame| draw_backup_selector(frame, selector))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => selector.previous(),
            KeyCode::Down | KeyCode::Char('j') => selector.next(),
            KeyCode::Char(' ') => selector.toggle_selected(),
            KeyCode::Enter => {
                if selector.selected.is_empty() {
                    selector.message = Some("至少选择一个 profile".to_string());
                    continue;
                }
                let selected: Vec<ProfileSummary> = selector
                    .profiles
                    .iter()
                    .filter(|p| selector.selected.contains(&p.id))
                    .cloned()
                    .collect();
                return Ok(Some(selected));
            }
            KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
            _ => {}
        }
    }
}

fn draw_backup_selector(frame: &mut ratatui::Frame<'_>, selector: &BackupSelectionState) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Codex Switch",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Backup Restore", Style::default().fg(Color::Gray)),
        ]),
        Line::from("空格多选，Enter 导入选中，q 或 Esc 退出"),
    ])
    .block(Block::default().borders(Borders::ALL).title("TUI"));
    frame.render_widget(header, layout[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[1]);

    let mut list_state = ListState::default().with_selected(Some(selector.selected_index));
    let items: Vec<ListItem<'_>> = selector
        .profiles
        .iter()
        .map(|profile| {
            let checked = if selector.selected.contains(&profile.id) { "■" } else { "□" };
            let exists_mark = if selector.is_existing(&profile.id) { "⚠" } else { " " };
            ListItem::new(Line::from(vec![
                Span::styled(checked, Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(exists_mark, Style::default().fg(Color::Red)),
                Span::raw(" "),
                Span::styled(&profile.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(
                    format!("({})", profile.id),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Backup Profiles"))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, body[0], &mut list_state);

    let detail = Paragraph::new(selector.detail_lines())
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Detail"));
    frame.render_widget(detail, body[1]);

    let footer =
        Paragraph::new("↑/↓ 或 j/k 移动  Space 勾选  Enter 导入  q/Esc 退出  ⚠=本地已存在")
            .block(Block::default().borders(Borders::ALL).title("Keys"));
    frame.render_widget(footer, layout[2]);

    frame.set_cursor_position((0, 0));
}

fn run_selector(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    selector: &mut ProfileSelectorState,
) -> anyhow::Result<Option<ProfileSummary>> {
    loop {
        terminal.draw(|frame| draw_selector(frame, selector))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => selector.previous(),
            KeyCode::Down | KeyCode::Char('j') => selector.next(),
            KeyCode::Enter => return Ok(selector.selected_profile().cloned()),
            KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
            _ => {}
        }
    }
}

fn run_delete_selector(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    selector: &mut DeleteSelectionState,
) -> anyhow::Result<Option<Vec<ProfileSummary>>> {
    let mut confirm = None;

    loop {
        terminal.draw(|frame| draw_delete_selector(frame, selector, confirm.as_ref()))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if confirm.is_some() {
            match key.code {
                KeyCode::Enter => return Ok(confirm.map(|value: DeleteConfirmState| value.selected_profiles)),
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                    confirm = None;
                    selector.message = None;
                }
                _ => {}
            }
            continue;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => selector.previous(),
            KeyCode::Down | KeyCode::Char('j') => selector.next(),
            KeyCode::Char(' ') => selector.toggle_selected(),
            KeyCode::Enter => {
                if selector.selected.is_empty() {
                    selector.message = Some("至少选择一个 profile".to_string());
                    continue;
                }

                let next_confirm = DeleteConfirmState::new(&selector.profiles, &selector.selected);
                if next_confirm.has_active_profile {
                    selector.message = Some(next_confirm.confirmation_error());
                    continue;
                }

                selector.message = None;
                confirm = Some(next_confirm);
            }
            KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
            _ => {}
        }
    }
}

fn draw_selector(frame: &mut ratatui::Frame<'_>, selector: &ProfileSelectorState) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Codex Switch",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Profile Selector", Style::default().fg(Color::Gray)),
        ]),
        Line::from("选择一个 profile，按 Enter 切换，按 q 或 Esc 退出"),
    ])
    .block(Block::default().borders(Borders::ALL).title("TUI"));
    frame.render_widget(header, layout[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[1]);

    let mut list_state = ListState::default().with_selected(Some(selector.selected));
    let items: Vec<ListItem<'_>> = selector
        .profiles
        .iter()
        .map(|profile| {
            let marker = if profile.active { "●" } else { "○" };
            ListItem::new(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default().fg(if profile.active {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(" "),
                Span::styled(&profile.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(
                    format!("({})", profile.id),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Profiles"))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, body[0], &mut list_state);

    let detail = selector.detail_lines();
    let detail = Paragraph::new(detail)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Detail"));
    frame.render_widget(detail, body[1]);

    let footer = Paragraph::new("↑/↓ 或 j/k 移动  Enter 切换  q/Esc 退出")
        .block(Block::default().borders(Borders::ALL).title("Keys"));
    frame.render_widget(footer, layout[2]);

    frame.set_cursor_position((0, 0));
}

fn draw_delete_selector(
    frame: &mut ratatui::Frame<'_>,
    selector: &DeleteSelectionState,
    confirm: Option<&DeleteConfirmState>,
) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Codex Switch",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Profile Delete", Style::default().fg(Color::Gray)),
        ]),
        Line::from(match confirm {
            Some(_) => "确认即将删除的 profile，按 Enter 确认，按 Esc 返回",
            None => "空格多选，Enter 进入确认，按 q 或 Esc 退出",
        }),
    ])
    .block(Block::default().borders(Borders::ALL).title("TUI"));
    frame.render_widget(header, layout[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[1]);

    let mut list_state = ListState::default().with_selected(Some(selector.selected_index));
    let items: Vec<ListItem<'_>> = selector
        .profiles
        .iter()
        .map(|profile| {
            let checked = if selector.selected.contains(&profile.id) { "■" } else { "□" };
            let active = if profile.active { "●" } else { "○" };
            ListItem::new(Line::from(vec![
                Span::styled(checked, Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(
                    active,
                    Style::default().fg(if profile.active {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(" "),
                Span::styled(&profile.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(
                    format!("({})", profile.id),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Profiles"))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, body[0], &mut list_state);

    let detail = match confirm {
        Some(confirm) => Paragraph::new(confirm.detail_lines())
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Confirm")),
        None => Paragraph::new(selector.detail_lines())
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Detail")),
    };
    frame.render_widget(detail, body[1]);

    let footer_text = match confirm {
        Some(_) => "Enter 确认删除  Backspace/Esc 返回",
        None => selector.footer_text(),
    };
    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL).title("Keys"));
    frame.render_widget(footer, layout[2]);

    frame.set_cursor_position((0, 0));
}

#[derive(Debug)]
struct ProfileSelectorState {
    profiles: Vec<ProfileSummary>,
    selected: usize,
}

#[derive(Debug)]
struct DeleteSelectionState {
    profiles: Vec<ProfileSummary>,
    selected_index: usize,
    selected: BTreeSet<String>,
    message: Option<String>,
}

impl DeleteSelectionState {
    fn new(profiles: Vec<ProfileSummary>) -> Self {
        Self {
            profiles,
            selected_index: 0,
            selected: BTreeSet::new(),
            message: None,
        }
    }

    fn next(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.profiles.len();
    }

    fn previous(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            self.profiles.len() - 1
        } else {
            self.selected_index - 1
        };
    }

    fn toggle_selected(&mut self) {
        let Some(profile) = self.profiles.get(self.selected_index) else {
            return;
        };

        if !self.selected.remove(&profile.id) {
            self.selected.insert(profile.id.clone());
        }
        self.message = None;
    }

    #[cfg(test)]
    fn selected_ids(&self) -> Vec<String> {
        self.selected.iter().cloned().collect()
    }

    fn detail_lines(&self) -> Vec<Line<'static>> {
        let Some(profile) = self.profiles.get(self.selected_index) else {
            return vec![Line::from("暂无 profile")];
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("名称: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.name.clone()),
            ]),
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.id.clone()),
            ]),
            Line::from(vec![
                Span::styled("邮箱: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.email.clone().unwrap_or_default()),
            ]),
            Line::from(""),
            Line::from(format!("已选中: {} 个", self.selected.len())),
        ];

        if let Some(message) = &self.message {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("提示: ", Style::default().fg(Color::Red)),
                Span::raw(message.clone()),
            ]));
        }

        lines
    }

    fn footer_text(&self) -> &str {
        "↑/↓ 或 j/k 移动  Space 勾选  Enter 确认页  q/Esc 退出"
    }
}

#[derive(Debug, Clone)]
struct DeleteConfirmState {
    selected_profiles: Vec<ProfileSummary>,
    has_active_profile: bool,
}

impl DeleteConfirmState {
    fn new(profiles: &[ProfileSummary], selected: &BTreeSet<String>) -> Self {
        let selected_profiles = profiles
            .iter()
            .filter(|profile| selected.contains(&profile.id))
            .cloned()
            .collect::<Vec<_>>();
        let has_active_profile = selected_profiles.iter().any(|profile| profile.active);

        Self {
            selected_profiles,
            has_active_profile,
        }
    }

    fn confirmation_error(&self) -> String {
        "当前激活的 profile 不允许删除，请先切换到其他 profile".to_string()
    }

    fn detail_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from("以下 profile 将被永久删除:")];
        for profile in &self.selected_profiles {
            lines.push(Line::from(format!("- {} ({})", profile.name, profile.id)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("按 Enter 确认删除，按 Esc 返回继续选择"));
        lines
    }
}

struct BackupSelectionState {
    profiles: Vec<ProfileSummary>,
    selected_index: usize,
    selected: BTreeSet<String>,
    existing: std::collections::HashSet<String>,
    message: Option<String>,
}

impl BackupSelectionState {
    fn new(profiles: Vec<ProfileSummary>, existing: std::collections::HashSet<String>) -> Self {
        Self {
            profiles,
            selected_index: 0,
            selected: BTreeSet::new(),
            existing,
            message: None,
        }
    }

    fn is_existing(&self, id: &str) -> bool {
        self.existing.contains(id)
    }

    fn next(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.profiles.len();
    }

    fn previous(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            self.profiles.len() - 1
        } else {
            self.selected_index - 1
        };
    }

    fn toggle_selected(&mut self) {
        let Some(profile) = self.profiles.get(self.selected_index) else {
            return;
        };
        if self.is_existing(&profile.id) {
            self.message = Some("该 profile 本地已存在，导入会跳过".to_string());
            return;
        }
        if !self.selected.remove(&profile.id) {
            self.selected.insert(profile.id.clone());
        }
        self.message = None;
    }

    fn detail_lines(&self) -> Vec<Line<'static>> {
        let Some(profile) = self.profiles.get(self.selected_index) else {
            return vec![Line::from("暂无 profile")];
        };

        let existing_note = if self.is_existing(&profile.id) {
            Some("本地已存在，将跳过")
        } else {
            None
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("名称: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.name.clone()),
            ]),
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.id.clone()),
            ]),
            Line::from(vec![
                Span::styled("邮箱: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.email.clone().unwrap_or_default()),
            ]),
            Line::from(""),
            Line::from(format!("已选中: {} 个", self.selected.len())),
        ];

        if let Some(note) = existing_note {
            lines.push(Line::from(vec![
                Span::styled("状态: ", Style::default().fg(Color::Red)),
                Span::raw(note),
            ]));
        }

        if let Some(msg) = &self.message {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("提示: ", Style::default().fg(Color::Red)),
                Span::raw(msg.clone()),
            ]));
        }
        lines
    }
}

impl ProfileSelectorState {
    fn new(profiles: Vec<ProfileSummary>) -> Self {
        let selected = profiles
            .iter()
            .position(|profile| profile.active)
            .unwrap_or(0);
        Self { profiles, selected }
    }

    fn next(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.profiles.len();
    }

    fn previous(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.profiles.len() - 1
        } else {
            self.selected - 1
        };
    }

    fn selected_profile(&self) -> Option<&ProfileSummary> {
        self.profiles.get(self.selected)
    }

    fn detail_lines(&self) -> Vec<Line<'static>> {
        let Some(profile) = self.selected_profile() else {
            return vec![Line::from("暂无 profile")];
        };

        vec![
            Line::from(vec![
                Span::styled("名称: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.name.clone()),
            ]),
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.id.clone()),
            ]),
            Line::from(vec![
                Span::styled("邮箱: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.email.clone().unwrap_or_default()),
            ]),
            Line::from(vec![
                Span::styled("订阅等级: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.subscription_plan.clone().unwrap_or_default()),
            ]),
            Line::from(vec![
                Span::styled("账号 ID: ", Style::default().fg(Color::Yellow)),
                Span::raw(profile.account_id.clone().unwrap_or_default()),
            ]),
            Line::from(""),
            Line::from(if profile.active {
                "当前状态: 已激活"
            } else {
                "当前状态: 未激活"
            }),
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// edit_config_fields: 通用 TUI 配置编辑表单
// ─────────────────────────────────────────────────────────────────────────────

/// 用 TUI 表单方式编辑一组配置字段。
/// fields: `(标签, 初始值, 是否敏感, 提示文本)`
/// 提示文本可用 `\n` 换行。返回 Some(values) 表示用户保存；None 表示取消。
pub fn edit_config_fields(
    title: &'static str,
    fields: Vec<(&'static str, String, bool, &'static str)>,
) -> anyhow::Result<Option<Vec<String>>> {
    if !io::stdout().is_terminal() {
        anyhow::bail!("交互式 TUI 需要在真实终端中运行");
    }
    let mut state = ConfigEditorState::new(title, fields);
    with_tui(|terminal| run_config_editor(terminal, &mut state))
}

/// 让用户从备份文件列表中选一个。返回 None 表示取消。
pub fn select_backup_file(files: Vec<String>) -> anyhow::Result<Option<String>> {
    if files.is_empty() {
        return Ok(None);
    }
    if !io::stdout().is_terminal() {
        anyhow::bail!("交互式 TUI 需要在真实终端中运行");
    }
    let mut state = BackupFileState::new(files);
    with_tui(|terminal| run_file_selector(terminal, &mut state))
}

/// 用 TUI 单行输入框输入口令（输入内容用 `*` 掩码）。返回 None 表示取消。
pub fn input_password(prompt: &'static str) -> anyhow::Result<Option<String>> {
    if !io::stdout().is_terminal() {
        anyhow::bail!("交互式 TUI 需要在真实终端中运行");
    }
    let mut state = PasswordInputState::new(prompt);
    with_tui(|terminal| run_password_input(terminal, &mut state))
}

fn run_config_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut ConfigEditorState,
) -> anyhow::Result<Option<Vec<String>>> {
    loop {
        terminal.draw(|frame| draw_config_editor(frame, state))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if state.edit_buf.is_some() {
            match key.code {
                KeyCode::Enter => state.confirm_edit(),
                // Tab: 确认当前字段并自动跳到下一个字段开始编辑
                KeyCode::Tab => {
                    state.confirm_edit();
                    if state.selected + 1 < state.labels.len() {
                        state.selected += 1;
                        state.start_edit();
                    }
                }
                KeyCode::Esc => state.cancel_edit(),
                KeyCode::Backspace => {
                    if let Some(buf) = &mut state.edit_buf {
                        buf.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(buf) = &mut state.edit_buf {
                        buf.push(c);
                    }
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.selected > 0 {
                        state.selected -= 1;
                    }
                    state.message = None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.selected + 1 < state.labels.len() {
                        state.selected += 1;
                    }
                    state.message = None;
                }
                KeyCode::Enter => state.start_edit(),
                KeyCode::Char('s') => {
                    if state.values[0].trim().is_empty() {
                        state.message = Some("第一个字段（WebDAV URL）不能为空".to_string());
                        continue;
                    }
                    return Ok(Some(state.values.clone()));
                }
                KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                _ => {}
            }
        }
    }
}

fn draw_config_editor(frame: &mut ratatui::Frame<'_>, state: &ConfigEditorState) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    // 计算提示面板高度（hint 按 \n 分行 + 2 个边框行，最多 5 行）
    let hint_text = state.hints.get(state.selected).copied().unwrap_or("");
    let hint_lines_count = hint_text.split('\n').count();
    let hint_height = (hint_lines_count as u16 + 2).min(5);

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),           // header
            Constraint::Min(6),              // body: field list (2 rows per field)
            Constraint::Length(hint_height), // hint panel
            Constraint::Length(3),           // footer
        ])
        .split(area);

    // ── Header ────────────────────────────────────────────────────────────────
    let mode_hint = if state.edit_buf.is_some() {
        "编辑模式: Enter 确认  Tab 确认并跳到下一项  Esc 放弃"
    } else {
        "↑/↓ j/k 切换字段  Enter 编辑  s 保存  q/Esc 取消"
    };
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Codex Switch",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ──  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                state.title,
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![Span::styled(mode_hint, Style::default().fg(Color::DarkGray))]),
    ])
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, outer[0]);

    // ── Body: 全宽字段列表，每个字段占 2 行（字段名 + 输入值）─────────────────
    let mut list_state = ListState::default().with_selected(Some(state.selected));
    let items: Vec<ListItem<'_>> = state
        .labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_selected = i == state.selected;

            // 标签行样式
            let label_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };

            // 值行内容与样式
            let (val_text, val_style) = if is_selected {
                if let Some(buf) = &state.edit_buf {
                    let masked = if state.sensitive[i] {
                        "●".repeat(buf.len())
                    } else {
                        buf.clone()
                    };
                    (
                        format!("> {}▌", masked),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    let (d, s) = state.list_display(i);
                    (format!("> {}", d), s)
                }
            } else {
                state.list_display(i)
            };

            ListItem::new(ratatui::text::Text::from(vec![
                Line::from(Span::styled(label.to_string(), label_style)),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(val_text, val_style),
                ]),
            ]))
        })
        .collect();

    let list_title = format!("Fields ({}/{})", state.selected + 1, state.labels.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        // 仅设置背景色，保留各 span 的前景色
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, outer[1], &mut list_state);

    // ── Hint panel ────────────────────────────────────────────────────────────
    let hint_content: Vec<Line<'_>> = hint_text
        .split('\n')
        .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Gray))))
        .collect();
    let hint_widget = Paragraph::new(hint_content)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("提示"));
    frame.render_widget(hint_widget, outer[2]);

    // ── Footer ────────────────────────────────────────────────────────────────
    let (footer_text, footer_style) = match &state.message {
        Some(msg) => (
            msg.as_str(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        None => ("s 保存  q/Esc 取消", Style::default().fg(Color::DarkGray)),
    };
    let footer = Paragraph::new(Line::from(vec![Span::styled(footer_text, footer_style)]))
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(footer, outer[3]);

    frame.set_cursor_position((0, 0));
}

fn run_file_selector(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut BackupFileState,
) -> anyhow::Result<Option<String>> {
    loop {
        terminal.draw(|frame| draw_file_selector(frame, state))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => state.previous(),
            KeyCode::Down | KeyCode::Char('j') => state.next(),
            KeyCode::Enter => return Ok(Some(state.files[state.selected].clone())),
            KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
            _ => {}
        }
    }
}

fn draw_file_selector(frame: &mut ratatui::Frame<'_>, state: &BackupFileState) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Codex Switch",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Select Backup", Style::default().fg(Color::Gray)),
        ]),
        Line::from("选择要恢复的备份文件，Enter 确认，q/Esc 取消"),
    ])
    .block(Block::default().borders(Borders::ALL).title("TUI"));
    frame.render_widget(header, layout[0]);

    let mut list_state = ListState::default().with_selected(Some(state.selected));
    let items: Vec<ListItem<'_>> = state
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let suffix = if i == 0 { "  (最新)" } else { "" };
            ListItem::new(Line::from(vec![
                Span::raw(f.clone()),
                Span::styled(suffix, Style::default().fg(Color::Green)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Backup Files"))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, layout[1], &mut list_state);

    let footer = Paragraph::new("↑/↓ 或 j/k 移动  Enter 选择  q/Esc 取消")
        .block(Block::default().borders(Borders::ALL).title("Keys"));
    frame.render_widget(footer, layout[2]);

    frame.set_cursor_position((0, 0));
}

fn run_password_input(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut PasswordInputState,
) -> anyhow::Result<Option<String>> {
    loop {
        terminal.draw(|frame| draw_password_input(frame, state))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Enter => return Ok(Some(state.buf.clone())),
            KeyCode::Esc => return Ok(None),
            KeyCode::Backspace => {
                state.buf.pop();
            }
            KeyCode::Char(c) => state.buf.push(c),
            _ => {}
        }
    }
}

fn draw_password_input(frame: &mut ratatui::Frame<'_>, state: &PasswordInputState) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Codex Switch",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(state.prompt),
    ])
    .block(Block::default().borders(Borders::ALL).title("TUI"));
    frame.render_widget(header, layout[0]);

    let masked = "*".repeat(state.buf.len());
    let input_bar = Paragraph::new(vec![Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Green)),
        Span::styled(format!("{}_", masked), Style::default().fg(Color::Green)),
    ])])
    .block(Block::default().borders(Borders::ALL).title("Input"));
    frame.render_widget(input_bar, layout[1]);

    let footer = Paragraph::new("Enter 确认  Esc 取消")
        .block(Block::default().borders(Borders::ALL).title("Keys"));
    frame.render_widget(footer, layout[2]);

    frame.set_cursor_position((0, 0));
}

struct ConfigEditorState {
    title: &'static str,
    labels: Vec<&'static str>,
    hints: Vec<&'static str>,
    values: Vec<String>,
    sensitive: Vec<bool>,
    selected: usize,
    edit_buf: Option<String>,
    message: Option<String>,
}

impl ConfigEditorState {
    fn new(title: &'static str, fields: Vec<(&'static str, String, bool, &'static str)>) -> Self {
        let mut labels = Vec::with_capacity(fields.len());
        let mut hints = Vec::with_capacity(fields.len());
        let mut values = Vec::with_capacity(fields.len());
        let mut sensitive = Vec::with_capacity(fields.len());
        for (label, value, is_sensitive, hint) in fields {
            labels.push(label);
            hints.push(hint);
            values.push(value);
            sensitive.push(is_sensitive);
        }
        Self {
            title,
            labels,
            hints,
            values,
            sensitive,
            selected: 0,
            edit_buf: None,
            message: None,
        }
    }

    fn start_edit(&mut self) {
        let current = self.values[self.selected].clone();
        self.edit_buf = Some(current);
        self.message = None;
    }

    fn confirm_edit(&mut self) {
        if let Some(buf) = self.edit_buf.take() {
            self.values[self.selected] = buf;
        }
    }

    fn cancel_edit(&mut self) {
        self.edit_buf = None;
    }

    /// 左侧列表中的简短显示（敏感字段用 ●，长值截断）
    fn list_display(&self, i: usize) -> (String, Style) {
        if self.values[i].is_empty() {
            return (
                "(空)".to_string(),
                Style::default().fg(Color::DarkGray),
            );
        }
        if self.sensitive[i] {
            return (
                "●●●●".to_string(),
                Style::default().fg(Color::DarkGray),
            );
        }
        let v = &self.values[i];
        let display = if v.chars().count() > 15 {
            format!("{}…", &v[..v.char_indices().nth(14).map(|(i, _)| i).unwrap_or(v.len())])
        } else {
            v.clone()
        };
        (display, Style::default().fg(Color::White))
    }

    /// 右侧详情面板的完整值 span（敏感字段仍掩码，但显示实际字符数）
    fn detail_value_span(&self, i: usize) -> Span<'static> {
        if self.values[i].is_empty() {
            Span::styled("(空)", Style::default().fg(Color::DarkGray))
        } else if self.sensitive[i] {
            Span::styled(
                "●".repeat(self.values[i].len()),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::styled(self.values[i].clone(), Style::default().fg(Color::White))
        }
    }
}

struct BackupFileState {
    files: Vec<String>,
    selected: usize,
}

impl BackupFileState {
    fn new(files: Vec<String>) -> Self {
        Self { files, selected: 0 }
    }

    fn next(&mut self) {
        if !self.files.is_empty() {
            self.selected = (self.selected + 1) % self.files.len();
        }
    }

    fn previous(&mut self) {
        if !self.files.is_empty() {
            self.selected = if self.selected == 0 {
                self.files.len() - 1
            } else {
                self.selected - 1
            };
        }
    }
}

struct PasswordInputState {
    prompt: &'static str,
    buf: String,
}

impl PasswordInputState {
    fn new(prompt: &'static str) -> Self {
        Self { prompt, buf: String::new() }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::model::ProfileSummary;

    use super::{BackupSelectionState, DeleteConfirmState, DeleteSelectionState, ProfileSelectorState};

    #[test]
    fn selector_defaults_to_active_profile() {
        let state = ProfileSelectorState::new(vec![
            ProfileSummary {
                id: "alpha".to_string(),
                name: "alpha".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: false,
            },
            ProfileSummary {
                id: "beta".to_string(),
                name: "beta".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: true,
            },
        ]);

        assert_eq!(
            state.selected_profile().map(|profile| profile.id.as_str()),
            Some("beta")
        );
    }

    #[test]
    fn selector_navigation_wraps() {
        let mut state = ProfileSelectorState::new(vec![
            ProfileSummary {
                id: "alpha".to_string(),
                name: "alpha".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: true,
            },
            ProfileSummary {
                id: "beta".to_string(),
                name: "beta".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: false,
            },
        ]);

        state.previous();
        assert_eq!(
            state.selected_profile().map(|profile| profile.id.as_str()),
            Some("beta")
        );

        state.next();
        assert_eq!(
            state.selected_profile().map(|profile| profile.id.as_str()),
            Some("alpha")
        );
    }

    #[test]
    fn delete_selector_toggles_multiple_profiles() {
        let mut state = DeleteSelectionState::new(vec![
            ProfileSummary {
                id: "alpha".to_string(),
                name: "alpha".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: false,
            },
            ProfileSummary {
                id: "beta".to_string(),
                name: "beta".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: false,
            },
        ]);

        state.toggle_selected();
        state.next();
        state.toggle_selected();

        assert_eq!(state.selected_ids(), vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn delete_confirm_blocks_active_profile() {
        let state = DeleteSelectionState::new(vec![
            ProfileSummary {
                id: "alpha".to_string(),
                name: "alpha".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: true,
            },
            ProfileSummary {
                id: "beta".to_string(),
                name: "beta".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: false,
            },
        ]);

        let selected = BTreeSet::from(["alpha".to_string(), "beta".to_string()]);
        let confirm = DeleteConfirmState::new(&state.profiles, &selected);

        assert!(confirm.has_active_profile);
        assert!(confirm.confirmation_error().contains("当前激活的 profile 不允许删除"));
    }

    #[test]
    fn backup_selector_marks_existing_profiles() {
        use std::collections::HashSet;

        let profiles = vec![
            ProfileSummary {
                id: "alice".to_string(),
                name: "alice".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: false,
            },
            ProfileSummary {
                id: "bob".to_string(),
                name: "bob".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                plan_type: None,
                primary: None,
                secondary: None,
                active: false,
            },
        ];
        let existing: HashSet<String> = ["bob".to_string()].into();
        let mut state = BackupSelectionState::new(profiles, existing);

        assert!(!state.is_existing("alice"));
        assert!(state.is_existing("bob"));

        // 可以选 alice
        state.toggle_selected();
        assert!(state.selected.contains("alice"));

        // bob 已存在，toggle 不会选中，但会设置 message
        state.next();
        state.toggle_selected();
        assert!(!state.selected.contains("bob"));
        assert!(state.message.is_some());
    }
}
