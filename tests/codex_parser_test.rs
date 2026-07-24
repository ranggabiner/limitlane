use limitlane::domain::types::*;
use limitlane::provider::codex::{normalize_codex_plan, CodexAdapter};

#[test]
fn test_parse_codex_plus_fixture() {
    let fixture = include_str!("fixtures/codex_plus.json");
    let adapter = CodexAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("personal@example.com", fixture)
        .expect("should parse codex plus fixture JSON successfully");

    assert_eq!(snapshot.account_id, "personal@example.com");
    assert_eq!(snapshot.provider, "codex");
    assert_eq!(snapshot.plan_raw.as_deref(), Some("plus"));

    let norm_plan = snapshot
        .plan_raw
        .as_deref()
        .map(normalize_codex_plan);
    assert_eq!(norm_plan, Some(NormalizedPlan::OpenAiPlus));

    // Verify shared agentic pool window
    let pool_w = snapshot
        .windows
        .iter()
        .find(|w| w.scope == UsageScope::SharedAgenticPool)
        .expect("shared agentic pool window missing");

    assert_eq!(pool_w.used_percent, Some(74.0));
    assert_eq!(pool_w.remaining_percent, Some(26.0));
    assert!(pool_w.reset_at.is_some());

    // Verify additional credit balance (1250 credits)
    let extra = snapshot
        .additional_usage
        .as_ref()
        .expect("additional usage should be present");
    assert_eq!(extra.enabled, true);
    assert_eq!(
        extra.balance.as_ref().map(|b| b.amount),
        Some(1250.0)
    );

    // Verify limiting window evaluation
    assert_eq!(
        snapshot.limiting_window_id.as_deref(),
        Some(pool_w.id.as_str())
    );
    assert_eq!(snapshot.effective_used_percent, Some(74.0));
    assert_eq!(snapshot.hard_limit_reached, false);
    assert_eq!(snapshot.availability, AccountAvailability::AvailableIncluded);
}

#[test]
fn test_dynamic_provider_defined_windows() {
    let json_data = serde_json::json!({
        "account_id": "pro@example.com",
        "plan": "pro",
        "windows": [
            {
                "id": "dyn_2h_session",
                "label": "2-Hour Burst Session",
                "window_type": "provider_defined",
                "duration_seconds": 7200,
                "used_percent": 35.0,
                "scope": "workspace"
            },
            {
                "id": "dyn_12h_rolling",
                "label": "12-Hour Cycle",
                "window_type": "rolling",
                "duration_seconds": 43200,
                "used_percent": 88.0,
                "scope": "shared_agentic_pool"
            }
        ]
    }).to_string();

    let adapter = CodexAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("pro@example.com", &json_data)
        .expect("should parse dynamic provider defined windows");

    assert_eq!(snapshot.windows.len(), 2);

    let burst_w = &snapshot.windows[0];
    assert_eq!(burst_w.window_type, WindowType::ProviderDefined);
    assert_eq!(burst_w.duration_seconds, Some(7200));
    assert_eq!(burst_w.used_percent, Some(35.0));

    let cycle_w = &snapshot.windows[1];
    assert_eq!(cycle_w.window_type, WindowType::Rolling);
    assert_eq!(cycle_w.duration_seconds, Some(43200));
    assert_eq!(cycle_w.used_percent, Some(88.0));

    assert_eq!(snapshot.limiting_window_id.as_deref(), Some(cycle_w.id.as_str()));
    assert_eq!(snapshot.effective_used_percent, Some(88.0));
}

#[test]
fn test_shared_pool_and_product_surface_scopes() {
    let json_data = serde_json::json!({
        "account_id": "team@example.com",
        "plan": "business",
        "windows": [
            {
                "id": "pool_win",
                "scope": "shared_agentic_pool",
                "used_percent": 50.0
            },
            {
                "id": "surface_win",
                "product_surface": "codex_cli",
                "used_percent": 20.0
            },
            {
                "id": "workspace_win",
                "scope": "workspace",
                "used_percent": 10.0
            }
        ]
    }).to_string();

    let adapter = CodexAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("team@example.com", &json_data)
        .expect("should parse pool and surface scopes");

    assert_eq!(snapshot.windows[0].scope, UsageScope::SharedAgenticPool);
    assert_eq!(
        snapshot.windows[1].scope,
        UsageScope::ProductSurface("codex_cli".to_string())
    );
    assert_eq!(snapshot.windows[2].scope, UsageScope::Workspace);
}

#[test]
fn test_token_based_credit_pricing_metadata_preservation() {
    let json_data = serde_json::json!({
        "account_id": "meta@example.com",
        "plan": "enterprise",
        "token_pricing_version": "2026.1",
        "windows": [
            {
                "id": "token_window",
                "used_percent": 65.0,
                "input_tokens_used": "1500000",
                "input_token_price": "$0.0015/1k",
                "output_token_price": "$0.0060/1k"
            }
        ]
    }).to_string();

    let adapter = CodexAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("meta@example.com", &json_data)
        .expect("should preserve token metadata");

    assert_eq!(
        snapshot.raw_metadata.get("token_pricing_version").map(|s| s.as_str()),
        Some("\"2026.1\"")
    );

    let win = &snapshot.windows[0];
    assert_eq!(
        win.provider_metadata.get("input_token_price").map(|s| s.as_str()),
        Some("\"$0.0015/1k\"")
    );
    assert_eq!(
        win.provider_metadata.get("output_token_price").map(|s| s.as_str()),
        Some("\"$0.0060/1k\"")
    );
}

#[test]
fn test_never_calculate_percentage_from_estimates() {
    let json_data = serde_json::json!({
        "account_id": "estimates@example.com",
        "plan": "pro",
        "windows": [
            {
                "id": "uncalculated_window",
                "estimated_tasks_used": 150,
                "estimated_tasks_limit": 300
                // used_percent intentionally omitted
            }
        ]
    }).to_string();

    let adapter = CodexAdapter::new_mock();
    let snapshot = adapter
        .parse_usage_json("estimates@example.com", &json_data)
        .expect("should parse missing used_percent without estimating");

    let win = &snapshot.windows[0];
    assert_eq!(win.used_percent, None);
    assert_eq!(win.remaining_percent, None);
}

#[test]
fn test_snapshot_limiting_window_evaluation_for_codex() {
    // 100% hard limit reached WITH extra credits available -> AvailablePaidOverflow
    let json_with_credits = serde_json::json!({
        "account_id": "capped_credits@example.com",
        "plan": "plus",
        "shared_agentic_pool": {
            "used_percent": 100.0,
            "reset_at": "2026-07-25T12:00:00Z"
        },
        "extra_credits": {
            "enabled": true,
            "exhausted": false,
            "balance": 500.0
        }
    }).to_string();

    let adapter = CodexAdapter::new_mock();
    let snapshot1 = adapter
        .parse_usage_json("capped_credits@example.com", &json_with_credits)
        .expect("should parse capped json with credits");

    assert_eq!(snapshot1.hard_limit_reached, true);
    assert_eq!(snapshot1.availability, AccountAvailability::AvailablePaidOverflow);

    // 100% hard limit reached WITHOUT extra credits -> Limited
    let json_no_credits = serde_json::json!({
        "account_id": "capped_no_credits@example.com",
        "plan": "plus",
        "shared_agentic_pool": {
            "used_percent": 100.0,
            "reset_at": "2026-07-25T12:00:00Z"
        },
        "extra_credits": {
            "enabled": true,
            "exhausted": true,
            "balance": 0.0
        }
    }).to_string();

    let snapshot2 = adapter
        .parse_usage_json("capped_no_credits@example.com", &json_no_credits)
        .expect("should parse capped json without credits");

    assert_eq!(snapshot2.hard_limit_reached, true);
    assert_eq!(snapshot2.availability, AccountAvailability::Limited);
}
