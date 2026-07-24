use limitlane::domain::types::*;
use limitlane::domain::evaluator::evaluate_limiting_window;
use chrono::Utc;
use std::collections::BTreeMap;

#[test]
fn test_evaluate_limiting_window_selects_highest_usage() {
    let w1 = UsageWindow {
        id: "w1".into(),
        raw_provider_id: None,
        label: "Session".into(),
        scope: UsageScope::AllAccountUsage,
        window_type: WindowType::Rolling,
        enforcement: EnforcementType::Hard,
        duration_seconds: Some(18000),
        used_percent: Some(40.0),
        remaining_percent: Some(60.0),
        consumed_units: None,
        limit_units: None,
        unit: Some(UsageUnit::Percent),
        reset_at: None,
        observed_at: Utc::now(),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        provider_metadata: BTreeMap::new(),
    };

    let w2 = UsageWindow {
        id: "w2".into(),
        used_percent: Some(85.0),
        remaining_percent: Some(15.0),
        ..w1.clone()
    };

    let mut snapshot = UsageSnapshot {
        account_id: "acc_1".into(),
        provider: "claude".into(),
        plan_raw: Some("pro".into()),
        observed_at: Utc::now(),
        windows: vec![w1, w2],
        limiting_window_id: None,
        effective_used_percent: None,
        hard_limit_reached: false,
        availability: AccountAvailability::AvailableIncluded,
        additional_usage: None,
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        raw_metadata: BTreeMap::new(),
    };

    evaluate_limiting_window(&mut snapshot);

    assert_eq!(snapshot.limiting_window_id.as_deref(), Some("w2"));
    assert_eq!(snapshot.effective_used_percent, Some(85.0));
    assert_eq!(snapshot.availability, AccountAvailability::AvailableIncluded);
    assert_eq!(snapshot.hard_limit_reached, false);
}

#[test]
fn test_paid_overflow_when_included_exhausted_but_credits_exist() {
    let w1 = UsageWindow {
        id: "w1".into(),
        raw_provider_id: None,
        label: "Session".into(),
        scope: UsageScope::AllAccountUsage,
        window_type: WindowType::Rolling,
        enforcement: EnforcementType::Hard,
        duration_seconds: Some(18000),
        used_percent: Some(100.0),
        remaining_percent: Some(0.0),
        consumed_units: None,
        limit_units: None,
        unit: Some(UsageUnit::Percent),
        reset_at: None,
        observed_at: Utc::now(),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        provider_metadata: BTreeMap::new(),
    };

    let mut snapshot = UsageSnapshot {
        account_id: "acc_1".into(),
        provider: "claude".into(),
        plan_raw: Some("pro".into()),
        observed_at: Utc::now(),
        windows: vec![w1],
        limiting_window_id: None,
        effective_used_percent: None,
        hard_limit_reached: false,
        availability: AccountAvailability::AvailableIncluded,
        additional_usage: Some(AdditionalUsage {
            enabled: true,
            exhausted: Some(false),
            balance: None,
            spent: None,
            spending_limit: None,
            used_percent: None,
            shared_scope: None,
        }),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        raw_metadata: BTreeMap::new(),
    };

    evaluate_limiting_window(&mut snapshot);

    assert_eq!(snapshot.hard_limit_reached, true);
    assert_eq!(snapshot.availability, AccountAvailability::AvailablePaidOverflow);
}

#[test]
fn test_setting_limited_when_included_exhausted_and_credits_unavailable_or_exhausted() {
    // Subtest A: additional_usage is None
    let w1 = UsageWindow {
        id: "w1".into(),
        raw_provider_id: None,
        label: "Session".into(),
        scope: UsageScope::AllAccountUsage,
        window_type: WindowType::Rolling,
        enforcement: EnforcementType::Hard,
        duration_seconds: Some(18000),
        used_percent: Some(100.0),
        remaining_percent: Some(0.0),
        consumed_units: None,
        limit_units: None,
        unit: Some(UsageUnit::Percent),
        reset_at: None,
        observed_at: Utc::now(),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        provider_metadata: BTreeMap::new(),
    };

    let mut snapshot1 = UsageSnapshot {
        account_id: "acc_1".into(),
        provider: "claude".into(),
        plan_raw: Some("pro".into()),
        observed_at: Utc::now(),
        windows: vec![w1.clone()],
        limiting_window_id: None,
        effective_used_percent: None,
        hard_limit_reached: false,
        availability: AccountAvailability::AvailableIncluded,
        additional_usage: None,
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        raw_metadata: BTreeMap::new(),
    };

    evaluate_limiting_window(&mut snapshot1);

    assert_eq!(snapshot1.hard_limit_reached, true);
    assert_eq!(snapshot1.availability, AccountAvailability::Limited);

    // Subtest B: additional_usage enabled = false
    let mut snapshot2 = UsageSnapshot {
        additional_usage: Some(AdditionalUsage {
            enabled: false,
            exhausted: Some(false),
            balance: None,
            spent: None,
            spending_limit: None,
            used_percent: None,
            shared_scope: None,
        }),
        ..snapshot1.clone()
    };

    evaluate_limiting_window(&mut snapshot2);

    assert_eq!(snapshot2.hard_limit_reached, true);
    assert_eq!(snapshot2.availability, AccountAvailability::Limited);

    // Subtest C: additional_usage enabled = true, but exhausted = Some(true)
    let mut snapshot3 = UsageSnapshot {
        additional_usage: Some(AdditionalUsage {
            enabled: true,
            exhausted: Some(true),
            balance: None,
            spent: None,
            spending_limit: None,
            used_percent: None,
            shared_scope: None,
        }),
        ..snapshot1.clone()
    };

    evaluate_limiting_window(&mut snapshot3);

    assert_eq!(snapshot3.hard_limit_reached, true);
    assert_eq!(snapshot3.availability, AccountAvailability::Limited);
}
