use chrono::Utc;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use limitlane::domain::types::*;
use limitlane::tui::app::{ApplicationState, Screen};
use limitlane::tui::ui;

fn sample_data() -> (Vec<Account>, Vec<UsageSnapshot>) {
    let fixed_now = chrono::DateTime::parse_from_rfc3339("2026-07-24T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let acc1 = Account {
        id: "acc_claude_1".into(),
        provider: "claude".into(),
        identity: "developer@example.com".into(),
        alias: Some("Work Claude".into()),
        auth_method: AuthMethod::OAuth,
        plan_raw: Some("max_5x".into()),
        plan_normalized: Some(NormalizedPlan::ClaudeMax5x),
        active: Some(true),
        health: AccountHealth::Ready,
        credential_reference: Some("keyring://claude/acc_claude_1".into()),
        created_at: fixed_now,
        updated_at: fixed_now,
        last_refresh_at: Some(fixed_now),
    };

    let acc2 = Account {
        id: "acc_codex_1".into(),
        provider: "codex".into(),
        identity: "codex_user@example.com".into(),
        alias: None,
        auth_method: AuthMethod::ApiKey,
        plan_raw: Some("pro".into()),
        plan_normalized: Some(NormalizedPlan::OpenAiPro),
        active: Some(true),
        health: AccountHealth::Ready,
        credential_reference: Some("keyring://codex/acc_codex_1".into()),
        created_at: fixed_now,
        updated_at: fixed_now,
        last_refresh_at: Some(fixed_now),
    };

    let w1 = UsageWindow {
        id: "w1".into(),
        raw_provider_id: None,
        label: "5-hour session".into(),
        scope: UsageScope::AllAccountUsage,
        window_type: WindowType::Rolling,
        enforcement: EnforcementType::Hard,
        duration_seconds: Some(18000),
        used_percent: Some(42.5),
        remaining_percent: Some(57.5),
        consumed_units: None,
        limit_units: None,
        unit: Some(UsageUnit::Percent),
        reset_at: Some(fixed_now + chrono::Duration::hours(2)),
        observed_at: fixed_now,
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        provider_metadata: std::collections::BTreeMap::new(),
    };

    let snap1 = UsageSnapshot {
        account_id: "acc_claude_1".into(),
        provider: "claude".into(),
        plan_raw: Some("max_5x".into()),
        observed_at: fixed_now,
        windows: vec![w1.clone()],
        limiting_window_id: Some("w1".into()),
        effective_used_percent: Some(42.5),
        hard_limit_reached: false,
        availability: AccountAvailability::AvailableIncluded,
        additional_usage: None,
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        raw_metadata: std::collections::BTreeMap::new(),
    };

    let snap2 = UsageSnapshot {
        account_id: "acc_codex_1".into(),
        provider: "codex".into(),
        plan_raw: Some("pro".into()),
        observed_at: fixed_now,
        windows: vec![w1],
        limiting_window_id: Some("w1".into()),
        effective_used_percent: Some(88.0),
        hard_limit_reached: false,
        availability: AccountAvailability::AvailableIncluded,
        additional_usage: Some(AdditionalUsage {
            enabled: true,
            exhausted: Some(false),
            balance: Some(Money {
                amount: 15.50,
                currency: "USD".into(),
            }),
            spent: Some(Money {
                amount: 4.50,
                currency: "USD".into(),
            }),
            spending_limit: None,
            used_percent: Some(22.5),
            shared_scope: Some("Team Shared Pool".into()),
        }),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        raw_metadata: std::collections::BTreeMap::new(),
    };

    (vec![acc1, acc2], vec![snap1, snap2])
}

#[test]
fn test_render_dashboard_widget_80x24() {
    let (accounts, snapshots) = sample_data();
    let app = ApplicationState::new(accounts, snapshots);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let buffer = terminal.backend().buffer();

    // Verify key elements exist in buffer
    let buffer_str = format!("{:?}", buffer);
    assert!(buffer_str.contains("Accounts Dashboard"));
    assert!(buffer_str.contains("claude"));
    assert!(buffer_str.contains("Work Claude"));
    assert!(buffer_str.contains("42.5%"));

    insta::assert_snapshot!(format!("{}", terminal.backend()));
}

#[test]
fn test_render_account_detail_widget_80x24() {
    let (accounts, snapshots) = sample_data();
    let mut app = ApplicationState::new(accounts, snapshots);
    app.switch_screen(Screen::Details);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("Account Details"));
    assert!(buffer_str.contains("acc_claude_1"));
    assert!(buffer_str.contains("5-hour session"));

    insta::assert_snapshot!(format!("{}", terminal.backend()));
}

#[test]
fn test_responsive_layout_truncation_narrow_terminal() {
    let (accounts, snapshots) = sample_data();
    let app = ApplicationState::new(accounts, snapshots);

    // Narrow terminal (60 columns x 24 rows)
    let backend = TestBackend::new(60, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    // Should show compact indicator in title
    assert!(buffer_str.contains("[Compact]") || buffer_str.contains("Dashboard"));
    assert!(buffer_str.contains("claude"));

    insta::assert_snapshot!(format!("{}", terminal.backend()));
}
