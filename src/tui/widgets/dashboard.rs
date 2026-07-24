use chrono::{DateTime, Utc};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::tui::app::ApplicationState;

pub fn render(frame: &mut Frame, area: Rect, state: &ApplicationState) {
    let narrow = area.width < 80;

    let header_cells = if narrow {
        vec!["Provider", "Account", "Used %", "Reset"]
    } else {
        vec!["Provider", "Account", "Plan", "Limiting Scope", "Used %", "Reset"]
    };

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

    let rows = filtered_accounts.iter().enumerate().map(|(idx, acc)| {
        let snapshot = state.snapshots.get(&acc.id);

        let provider_str = &acc.provider;
        let account_str = acc.alias.as_deref().unwrap_or(&acc.identity);

        let plan_str = acc
            .plan_raw
            .as_deref()
            .or_else(|| acc.plan_normalized.as_ref().map(|p| format_plan(p)))
            .unwrap_or("Unknown");

        let (scope_str, used_str, reset_str, used_val) = match snapshot {
            Some(snap) => {
                let limiting_win = snap.windows.iter().find(|w| {
                    snap.limiting_window_id.as_deref() == Some(&w.id)
                });

                let scope = limiting_win
                    .map(|w| w.label.as_str())
                    .unwrap_or("All Usage");

                let used = snap.effective_used_percent.map(|p| format!("{:.1}%", p));

                let reset = limiting_win
                    .and_then(|w| w.reset_at)
                    .map(|dt| format_reset_time(dt))
                    .unwrap_or_else(|| "Unknown".to_string());

                (scope.to_string(), used.unwrap_or_else(|| "Unknown".to_string()), reset, snap.effective_used_percent)
            }
            None => ("Unknown".to_string(), "Unknown".to_string(), "Unknown".to_string(), None),
        };

        let used_style = match used_val {
            Some(val) if val >= 90.0 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            Some(val) if val >= 75.0 => Style::default().fg(Color::LightRed),
            Some(val) if val >= 50.0 => Style::default().fg(Color::Yellow),
            Some(_) => Style::default().fg(Color::Green),
            None => Style::default().fg(Color::DarkGray),
        };

        let is_selected = state.selected_account().map(|a| a.id == acc.id).unwrap_or(idx == state.selected_account_index);

        let row_style = if is_selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        if narrow {
            Row::new(vec![
                Cell::from(truncate(provider_str, 10)),
                Cell::from(truncate(account_str, 16)),
                Cell::from(used_str).style(used_style),
                Cell::from(truncate(&reset_str, 12)),
            ])
            .style(row_style)
        } else {
            Row::new(vec![
                Cell::from(truncate(provider_str, 12)),
                Cell::from(truncate(account_str, 20)),
                Cell::from(truncate(plan_str, 14)),
                Cell::from(truncate(&scope_str, 18)),
                Cell::from(used_str).style(used_style),
                Cell::from(reset_str),
            ])
            .style(row_style)
        }
    });

    let widths = if narrow {
        vec![
            Constraint::Percentage(20),
            Constraint::Percentage(35),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
        ]
    } else {
        vec![
            Constraint::Percentage(14),
            Constraint::Percentage(24),
            Constraint::Percentage(16),
            Constraint::Percentage(20),
            Constraint::Percentage(12),
            Constraint::Percentage(14),
        ]
    };

    let title = format!(
        " Accounts Dashboard ({}) {}",
        filtered_accounts.len(),
        if narrow { "[Compact]" } else { "" }
    );

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default().fg(Color::White)),
        );

    frame.render_widget(table, area);
}

fn format_plan(plan: &crate::domain::types::NormalizedPlan) -> &'static str {
    match plan {
        crate::domain::types::NormalizedPlan::ClaudeFree => "Claude Free",
        crate::domain::types::NormalizedPlan::ClaudePro => "Claude Pro",
        crate::domain::types::NormalizedPlan::ClaudeMax5x => "Claude Max (5x)",
        crate::domain::types::NormalizedPlan::ClaudeMax20x => "Claude Max (20x)",
        crate::domain::types::NormalizedPlan::ClaudeTeam => "Claude Team",
        crate::domain::types::NormalizedPlan::ClaudeEnterprise => "Claude Enterprise",
        crate::domain::types::NormalizedPlan::OpenAiFree => "ChatGPT Free",
        crate::domain::types::NormalizedPlan::OpenAiGo => "ChatGPT Go",
        crate::domain::types::NormalizedPlan::OpenAiPlus => "ChatGPT Plus",
        crate::domain::types::NormalizedPlan::OpenAiPro => "ChatGPT Pro",
        crate::domain::types::NormalizedPlan::OpenAiBusiness => "ChatGPT Business",
        crate::domain::types::NormalizedPlan::OpenAiEnterprise => "ChatGPT Enterprise",
        crate::domain::types::NormalizedPlan::OpenAiEdu => "ChatGPT Edu",
        crate::domain::types::NormalizedPlan::Api => "API",
        crate::domain::types::NormalizedPlan::Unknown => "Unknown",
    }
}

fn format_reset_time(reset_at: DateTime<Utc>) -> String {
    let now = Utc::now();
    if reset_at <= now {
        "Reset now".to_string()
    } else {
        let diff = reset_at - now;
        let hours = diff.num_hours();
        let mins = diff.num_minutes() % 60;
        if hours > 24 {
            let days = hours / 24;
            format!("{}d {}h", days, hours % 24)
        } else if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s.chars().take(max_len).collect()
    } else {
        let mut truncated: String = s.chars().take(max_len - 3).collect();
        truncated.push_str("...");
        truncated
    }
}
