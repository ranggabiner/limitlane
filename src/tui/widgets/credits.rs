use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::tui::app::ApplicationState;

pub fn render(frame: &mut Frame, area: Rect, state: &ApplicationState) {
    let header_cells = vec![
        "Provider",
        "Account",
        "Credits Enabled",
        "Balance",
        "Spent",
        "Shared Scope",
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

    let filtered_accounts = state.filtered_accounts();

    let rows: Vec<Row> = filtered_accounts
        .iter()
        .map(|acc| {
            let snap = state.snapshots.get(&acc.id);
            let (enabled_str, balance_str, spent_str, scope_str) = match snap {
                Some(s) => match &s.additional_usage {
                    Some(add) => {
                        let bal = add
                            .balance
                            .as_ref()
                            .map(|b| format!("{:.2} {}", b.amount, b.currency))
                            .unwrap_or_else(|| "Unknown".to_string());
                        let sp = add
                            .spent
                            .as_ref()
                            .map(|b| format!("{:.2} {}", b.amount, b.currency))
                            .unwrap_or_else(|| "Unknown".to_string());
                        let sc = add.shared_scope.as_deref().unwrap_or("Account").to_string();
                        (
                            if add.enabled { "Yes" } else { "No" },
                            bal,
                            sp,
                            sc,
                        )
                    }
                    None => ("No", "N/A".to_string(), "N/A".to_string(), "None".to_string()),
                },
                None => ("Unknown", "Unknown".to_string(), "Unknown".to_string(), "Unknown".to_string()),
            };

            Row::new(vec![
                Cell::from(acc.provider.clone()),
                Cell::from(acc.alias.clone().unwrap_or_else(|| acc.identity.clone())),
                Cell::from(enabled_str),
                Cell::from(balance_str),
                Cell::from(spent_str),
                Cell::from(scope_str),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(15),
        Constraint::Percentage(25),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Additional Credits & Shared Pools ")
                .style(Style::default().fg(Color::Magenta)),
        );

    frame.render_widget(table, area);
}
