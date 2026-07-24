use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::ApplicationState;

pub fn render(frame: &mut Frame, area: Rect, state: &ApplicationState) {
    let selected_account = state.selected_account();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Metadata / Header block
            Constraint::Min(8),    // Windows Table block
            Constraint::Length(6), // Additional Usage & Freshness block
        ])
        .split(area);

    let (acc_title, meta_text) = match selected_account {
        Some(acc) => {
            let snap = state.snapshots.get(&acc.id);
            let health_str = format!("{:?}", acc.health);
            let last_ref = acc
                .last_refresh_at
                .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "Never".to_string());
            let source_str = snap
                .map(|s| format!("{:?}", s.source))
                .unwrap_or_else(|| "N/A".to_string());
            let confidence_str = snap
                .map(|s| format!("{:?}", s.confidence))
                .unwrap_or_else(|| "N/A".to_string());

            let text = format!(
                "ID: {} | Provider: {} | Identity: {}\nAlias: {}\nHealth: {} | Last Refresh: {}\nObservation Source: {} | Confidence: {}",
                acc.id,
                acc.provider,
                acc.identity,
                acc.alias.as_deref().unwrap_or("None"),
                health_str,
                last_ref,
                source_str,
                confidence_str
            );
            (format!(" Account Details: {} ", acc.id), text)
        }
        None => (
            " Account Details ".to_string(),
            "No account selected.".to_string(),
        ),
    };

    let meta_block = Paragraph::new(meta_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(acc_title)
            .style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(meta_block, chunks[0]);

    // Windows table
    let header_cells = vec!["Window Label", "Scope", "Type", "Enforcement", "Used %", "Reset At"];
    let header = Row::new(header_cells.into_iter().map(|h| {
        Cell::from(h).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    }))
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = match state.selected_snapshot() {
        Some(snap) => snap
            .windows
            .iter()
            .map(|win| {
                let used_str = win
                    .used_percent
                    .map(|p| format!("{:.1}%", p))
                    .unwrap_or_else(|| "Unknown".to_string());

                let reset_str = win
                    .reset_at
                    .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let is_limiting = snap.limiting_window_id.as_deref() == Some(&win.id);
                let row_style = if is_limiting {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(win.label.clone()),
                    Cell::from(format!("{:?}", win.scope)),
                    Cell::from(format!("{:?}", win.window_type)),
                    Cell::from(format!("{:?}", win.enforcement)),
                    Cell::from(used_str),
                    Cell::from(reset_str),
                ])
                .style(row_style)
            })
            .collect(),
        None => vec![],
    };

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(12),
        Constraint::Percentage(12),
        Constraint::Percentage(16),
    ];

    let windows_table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Limiting & Usage Windows ")
                .style(Style::default().fg(Color::White)),
        );
    frame.render_widget(windows_table, chunks[1]);

    // Additional Usage & Shared Pool Overview
    let add_usage_text = match state.selected_snapshot() {
        Some(snap) => match &snap.additional_usage {
            Some(add) => {
                let balance_str = add
                    .balance
                    .as_ref()
                    .map(|b| format!("{:.2} {}", b.amount, b.currency))
                    .unwrap_or_else(|| "N/A".to_string());
                let spent_str = add
                    .spent
                    .as_ref()
                    .map(|s| format!("{:.2} {}", s.amount, s.currency))
                    .unwrap_or_else(|| "N/A".to_string());
                let scope_str = add.shared_scope.as_deref().unwrap_or("None");

                format!(
                    "Additional Usage Enabled: {}\nExhausted: {:?}\nBalance: {} | Spent: {} | Scope: {}",
                    add.enabled, add.exhausted, balance_str, spent_str, scope_str
                )
            }
            None => "No additional usage or credit pool configured for this account.".to_string(),
        },
        None => "Snapshot unavailable.".to_string(),
    };

    let add_usage_block = Paragraph::new(add_usage_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Additional Usage & Shared Pool ")
            .style(Style::default().fg(Color::Green)),
    );
    frame.render_widget(add_usage_block, chunks[2]);
}
