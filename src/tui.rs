use std::{
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
    let mut stdout = io::stdout();
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal backend")?;

    let result = run_selector(&mut terminal, &mut selector);

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

#[derive(Debug)]
struct ProfileSelectorState {
    profiles: Vec<ProfileSummary>,
    selected: usize,
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
    use crate::model::ProfileSummary;

    use super::ProfileSelectorState;

    #[test]
    fn selector_defaults_to_active_profile() {
        let state = ProfileSelectorState::new(vec![
            ProfileSummary {
                id: "alpha".to_string(),
                name: "alpha".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                primary: None,
                active: false,
            },
            ProfileSummary {
                id: "beta".to_string(),
                name: "beta".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                primary: None,
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
                primary: None,
                active: true,
            },
            ProfileSummary {
                id: "beta".to_string(),
                name: "beta".to_string(),
                email: None,
                subscription_plan: None,
                account_id: None,
                primary: None,
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
}
