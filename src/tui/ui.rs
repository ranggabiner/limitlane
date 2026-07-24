use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame,
};

use crate::tui::app::{ApplicationState, Screen};
use crate::tui::widgets::{accounts, credits, dashboard, details, filters};

pub fn render(frame: &mut Frame, state: &ApplicationState) {
    let size = frame.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Navigation Tab Bar
            Constraint::Min(10),   // Main Content Area
            Constraint::Length(1), // Bottom Status / Keybindings Bar
        ])
        .split(size);

    // Top Navigation Tabs
    let titles = vec![
        "[1] Dashboard",
        "[2] Details",
        "[3] Credits",
        "[4] Accounts",
        "[5] Filters",
        "[?] Help",
    ];

    let selected_index = match state.current_screen {
        Screen::Dashboard => 0,
        Screen::Details => 1,
        Screen::Credits => 2,
        Screen::Accounts => 3,
        Screen::Filters => 4,
        Screen::Help => 5,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" LimitLane TUI v0.1.0 "),
        )
        .select(selected_index)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, chunks[0]);

    // Main Content
    match state.current_screen {
        Screen::Dashboard => dashboard::render(frame, chunks[1], state),
        Screen::Details => details::render(frame, chunks[1], state),
        Screen::Credits => credits::render(frame, chunks[1], state),
        Screen::Accounts => accounts::render(frame, chunks[1], state),
        Screen::Filters => filters::render(frame, chunks[1], state),
        Screen::Help => {
            dashboard::render(frame, chunks[1], state);
            render_help_overlay(frame, size);
        }
    }

    // Bottom Status Bar
    let status_text = if let Some(ref msg) = state.status_message {
        format!(" Status: {} ", msg)
    } else if state.is_refreshing {
        " Refreshing usage limit data... ".to_string()
    } else {
        " Keybindings: [j/k/↑/↓] Select | [Enter/d] Details | [Tab] Switch Tab | [r] Refresh | [/] Filter | [?] Help | [q] Quit ".to_string()
    };

    let status_bar = Paragraph::new(status_text).style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(status_bar, chunks[2]);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 60, area);

    let help_text = "\
LimitLane TUI Navigation & Shortcuts:

Navigation:
  j / Down Arrow   : Move to next account row
  k / Up Arrow     : Move to previous account row
  Enter / d        : View details for selected account
  Tab              : Cycle through views (Dashboard -> Details -> Credits -> Accounts -> Filters -> Help)
  1 - 5            : Jump directly to specific view

Actions:
  r / R            : Trigger refresh for selected account / all accounts
  a                : Switch to Accounts management view
  c                : Switch to Credits view
  f                : Switch to Filters view
  /                : Open filter / search input
  ? / h            : Toggle this Help popup
  Esc              : Close modal / Return to Dashboard / Clear search
  q / Ctrl+c       : Quit application
";

    let block = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Keyboard Shortcuts & Help ")
            .style(Style::default().fg(Color::Yellow).bg(Color::Reset)),
    );

    frame.render_widget(Clear, popup_area);
    frame.render_widget(block, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
