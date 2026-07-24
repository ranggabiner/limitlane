use limitlane::cli::commands::{format_accounts_json, format_status_json, format_usage_json};
use limitlane::domain::types::*;
use chrono::Utc;
use std::collections::BTreeMap;

#[test]
fn test_format_status_json_schema_v1() {
    let accounts = vec![Account {
        id: "acc_1".into(),
        provider: "claude".into(),
        identity: "user@example.com".into(),
        alias: Some("Work".into()),
        auth_method: AuthMethod::OAuth,
        plan_raw: Some("pro".into()),
        plan_normalized: Some(NormalizedPlan::ClaudePro),
        active: Some(true),
        health: AccountHealth::Ready,
        credential_reference: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_refresh_at: None,
    }];
    let snapshots = vec![];
    let json_str = format_status_json(&accounts, &snapshots).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["schema_version"], "1");
    assert!(val["accounts"].is_array());
}

#[test]
fn test_format_accounts_json_schema_v1() {
    let accounts = vec![Account {
        id: "acc_1".into(),
        provider: "claude".into(),
        identity: "user@example.com".into(),
        alias: None,
        auth_method: AuthMethod::ApiKey,
        plan_raw: None,
        plan_normalized: None,
        active: Some(true),
        health: AccountHealth::Ready,
        credential_reference: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_refresh_at: None,
    }];
    let json_str = format_accounts_json(&accounts).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["schema_version"], "1");
    assert_eq!(val["accounts"][0]["id"], "acc_1");
}

#[test]
fn test_format_usage_json_schema_v1_and_nulls() {
    let window = UsageWindow {
        id: "w1".into(),
        raw_provider_id: None,
        label: "Session".into(),
        scope: UsageScope::AllAccountUsage,
        window_type: WindowType::Rolling,
        enforcement: EnforcementType::Hard,
        duration_seconds: None,
        used_percent: None,
        remaining_percent: None,
        consumed_units: None,
        limit_units: None,
        unit: None,
        reset_at: None,
        observed_at: Utc::now(),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        provider_metadata: BTreeMap::new(),
    };

    let snapshot = UsageSnapshot {
        account_id: "acc_1".into(),
        provider: "claude".into(),
        plan_raw: None,
        observed_at: Utc::now(),
        windows: vec![window],
        limiting_window_id: None,
        effective_used_percent: None,
        hard_limit_reached: false,
        availability: AccountAvailability::AvailableIncluded,
        additional_usage: None,
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        raw_metadata: BTreeMap::new(),
    };

    let snapshots = vec![snapshot];
    let json_str = format_usage_json(&snapshots).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(val["schema_version"], "1");
    let window_val = &val["snapshots"][0]["windows"][0];

    // Verify missing values serialize as null, never 0 or missing key
    assert!(window_val["used_percent"].is_null());
    assert!(window_val["remaining_percent"].is_null());
    assert!(window_val["reset_at"].is_null());
    assert_ne!(window_val["used_percent"], 0);
}
