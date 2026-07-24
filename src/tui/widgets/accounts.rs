use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::tui::app::ApplicationState;

pub fn render(frame: &mut Frame, area: Rect, state: &ApplicationState) {
    let header_cells = vec![
        "ID",
        "Provider",
        "Identity",
        "Alias",
        "Auth Method",
        "Health Status",
        "Credential Ref",
    ];

    let header = Row::new(header_cells.into_iter().map(|h| {
        Cell::from(h).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    }))
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = state
        .accounts
        .iter()
        .map(|acc| {
            let auth_str = format!("{:?}", acc.auth_method);
            let health_str = format!("{:?}", acc.health);
            let cred_str = acc.credential_reference.as_deref().unwrap_or("None");

            let is_selected = state
                .selected_account()
                .map(|a| a.id == acc.id)
                .unwrap_or(false);

            let row_style = if is_selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(acc.id.clone()),
                Cell::from(acc.provider.clone()),
                Cell::from(acc.identity.clone()),
                Cell::from(acc.alias.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(auth_str),
                Cell::from(health_str),
                Cell::from(cred_str.to_string()),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Percentage(15),
        Constraint::Percentage(12),
        Constraint::Percentage(22),
        Constraint::Percentage(15),
        Constraint::Percentage(12),
        Constraint::Percentage(12),
        Constraint::Percentage(12),
    ];

    let title = format!(" Configured Accounts ({}) ", state.accounts.len());

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().fg(Color::Blue)),
        );

    frame.render_widget(table, area);
}
