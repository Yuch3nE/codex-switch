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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::model::ProfileSummary;

    use super::{DeleteConfirmState, DeleteSelectionState, ProfileSelectorState};

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
}
