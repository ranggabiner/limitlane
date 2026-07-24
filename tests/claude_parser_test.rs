use limitlane::domain::types::*;
use limitlane::provider::claude::ClaudeAdapter;

#[test]
fn test_parse_claude_pro_fixture() {
    let fixture = include_str!("fixtures/claude_pro.json");
    let adapter = ClaudeAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("dev@example.com", fixture)
        .expect("should parse fixture JSON successfully");

    assert_eq!(snapshot.account_id, "dev@example.com");
    assert_eq!(snapshot.provider, "claude");
    assert_eq!(snapshot.plan_raw.as_deref(), Some("max_5x"));

    // Verify 3 windows parsed
    assert_eq!(snapshot.windows.len(), 3);

    // 1. 5-hour session window
    let session_w = snapshot
        .windows
        .iter()
        .find(|w| w.id.contains("5h") || w.label.contains("5-hour"))
        .expect("5-hour session window missing");
    assert_eq!(session_w.window_type, WindowType::Rolling);
    assert_eq!(session_w.duration_seconds, Some(18000));
    assert_eq!(session_w.used_percent, Some(43.0));
    assert_eq!(session_w.remaining_percent, Some(57.0));
    assert_eq!(session_w.scope, UsageScope::AllAccountUsage);

    // 2. Weekly all-models window
    let weekly_all_w = snapshot
        .windows
        .iter()
        .find(|w| w.id.contains("weekly_all"))
        .expect("weekly all-models window missing");
    assert_eq!(weekly_all_w.window_type, WindowType::Fixed);
    assert_eq!(weekly_all_w.duration_seconds, Some(604800));
    assert_eq!(weekly_all_w.used_percent, Some(61.0));
    assert_eq!(weekly_all_w.remaining_percent, Some(39.0));
    assert_eq!(weekly_all_w.scope, UsageScope::AllModels);

    // 3. Weekly Sonnet window
    let weekly_sonnet_w = snapshot
        .windows
        .iter()
        .find(|w| w.id.contains("weekly_sonnet"))
        .expect("weekly sonnet window missing");
    assert_eq!(weekly_sonnet_w.window_type, WindowType::Fixed);
    assert_eq!(weekly_sonnet_w.duration_seconds, Some(604800));
    assert_eq!(weekly_sonnet_w.used_percent, Some(82.0));
    assert_eq!(weekly_sonnet_w.remaining_percent, Some(18.0));
    assert_eq!(
        weekly_sonnet_w.scope,
        UsageScope::ModelFamily("sonnet".to_string())
    );

    // Verify extra credits
    let extra = snapshot.additional_usage.as_ref().expect("extra credits expected");
    assert_eq!(extra.enabled, true);
    assert_eq!(extra.exhausted, Some(false));

    // Verify limiting window evaluation (highest used_percent is weekly_sonnet at 82%)
    assert_eq!(snapshot.limiting_window_id.as_deref(), Some(weekly_sonnet_w.id.as_str()));
    assert_eq!(snapshot.effective_used_percent, Some(82.0));
    assert_eq!(snapshot.hard_limit_reached, false);
    assert_eq!(snapshot.availability, AccountAvailability::AvailableIncluded);
}

#[test]
fn test_parse_unknown_model_scoped_windows() {
    let json_data = serde_json::json!({
        "account_id": "user@example.com",
        "plan": "pro",
        "limits": [
            {
                "name": "weekly_haiku",
                "used_percent": 15.0,
                "reset_at": "2026-07-31T00:00:00Z"
            },
            {
                "name": "monthly_opus",
                "used_percent": 95.0,
                "reset_at": "2026-08-01T00:00:00Z"
            }
        ]
    }).to_string();

    let adapter = ClaudeAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("user@example.com", &json_data)
        .expect("should parse json with unknown model scopes");

    assert_eq!(snapshot.windows.len(), 2);

    let haiku_w = &snapshot.windows[0];
    assert_eq!(haiku_w.scope, UsageScope::ModelFamily("haiku".to_string()));
    assert_eq!(haiku_w.window_type, WindowType::Fixed);
    assert_eq!(haiku_w.duration_seconds, Some(604800));

    let opus_w = &snapshot.windows[1];
    assert_eq!(opus_w.scope, UsageScope::ModelFamily("opus".to_string()));
    assert_eq!(opus_w.window_type, WindowType::Fixed);
    assert_eq!(opus_w.duration_seconds, Some(2592000));

    assert_eq!(snapshot.limiting_window_id.as_deref(), Some(opus_w.id.as_str()));
    assert_eq!(snapshot.effective_used_percent, Some(95.0));
}

#[test]
fn test_parse_missing_percentage_or_reset_time() {
    let json_data = serde_json::json!({
        "account_id": "no_details@example.com",
        "plan": "free",
        "limits": [
            {
                "name": "5h_session"
                // used_percent and reset_at missing
            },
            {
                "name": "weekly_all_model",
                "used_percent": 50.0
                // reset_at missing
            }
        ]
    }).to_string();

    let adapter = ClaudeAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("no_details@example.com", &json_data)
        .expect("should parse json with missing fields");

    let session_w = &snapshot.windows[0];
    assert_eq!(session_w.used_percent, None);
    assert_eq!(session_w.remaining_percent, None);
    assert_eq!(session_w.reset_at, None);

    let weekly_w = &snapshot.windows[1];
    assert_eq!(weekly_w.used_percent, Some(50.0));
    assert_eq!(weekly_w.remaining_percent, Some(50.0));
    assert_eq!(weekly_w.reset_at, None);
}

#[test]
fn test_claude_snapshot_limiting_window_evaluation() {
    let json_data = serde_json::json!({
        "account_id": "capped@example.com",
        "plan": "pro",
        "limits": [
            {
                "name": "5h_session",
                "used_percent": 100.0,
                "reset_at": "2026-07-24T18:00:00Z"
            },
            {
                "name": "weekly_all_model",
                "used_percent": 75.0,
                "reset_at": "2026-07-30T00:00:00Z"
            }
        ],
        "extra_credits": {
            "enabled": true,
            "exhausted": false
        }
    }).to_string();

    let adapter = ClaudeAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("capped@example.com", &json_data)
        .expect("should parse capped account json");

    assert_eq!(snapshot.hard_limit_reached, true);
    assert_eq!(snapshot.availability, AccountAvailability::AvailablePaidOverflow);
}
