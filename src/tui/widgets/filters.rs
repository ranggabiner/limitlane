use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::app::ApplicationState;

pub fn render(frame: &mut Frame, area: Rect, state: &ApplicationState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Search input box
            Constraint::Length(5), // Active filters summary
            Constraint::Min(4),    // Filter usage instructions
        ])
        .split(area);

    let search_title = if state.search_input_active {
        " Search Account (ACTIVE - type text, Enter/Esc to finish) "
    } else {
        " Search Account (Press '/' to activate) "
    };

    let search_style = if state.search_input_active {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let search_p = Paragraph::new(state.filters.account_search.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(search_title)
            .style(search_style),
    );
    frame.render_widget(search_p, chunks[0]);

    let provider_filter_str = state
        .filters
        .provider_filter
        .as_deref()
        .unwrap_or("All Providers");

    let min_usage_str = state
        .filters
        .min_usage_percent
        .map(|p| format!("{:.0}%", p))
        .unwrap_or_else(|| "None".to_string());

    let summary_text = format!(
        "Provider Filter: {}\nSearch Term: {}\nMin Usage Threshold: {}\nMatching Accounts: {} / {}",
        provider_filter_str,
        if state.filters.account_search.is_empty() { "None" } else { &state.filters.account_search },
        min_usage_str,
        state.filtered_accounts().len(),
        state.accounts.len()
    );

    let summary_p = Paragraph::new(summary_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Active Filters Summary ")
            .style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(summary_p, chunks[1]);

    let instructions = "Filter Navigation Controls:\n\n\
        • '/'       - Activate account search text field\n\
        • 'Esc'     - Clear active search / Reset filters / Exit filter screen\n\
        • '1'-'5'   - Quick jump between views\n\
        • 'q'       - Quit application";

    let help_p = Paragraph::new(instructions).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Instructions ")
            .style(Style::default().fg(Color::Gray)),
    );
    frame.render_widget(help_p, chunks[2]);
}
