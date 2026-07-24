use limitlane::domain::types::*;
use limitlane::storage::db::DatabaseRepository;
use limitlane::storage::keyring::KeyringStore;
use chrono::Utc;
use std::collections::BTreeMap;
use tempfile::NamedTempFile;

#[test]
fn test_database_account_lifecycle() {
    let tmp = NamedTempFile::new().unwrap();
    let db = DatabaseRepository::open(tmp.path()).unwrap();
    db.migrate().unwrap();

    let account = Account {
        id: "acc_claude_1".into(),
        provider: "claude".into(),
        identity: "dev@example.com".into(),
        alias: Some("Work Claude".into()),
        auth_method: AuthMethod::OAuth,
        plan_raw: Some("max_5x".into()),
        plan_normalized: Some(NormalizedPlan::ClaudeMax5x),
        active: Some(true),
        health: AccountHealth::Ready,
        credential_reference: Some("keyring://claude/acc_claude_1".into()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_refresh_at: None,
    };

    // 1. Save account
    db.save_account(&account).unwrap();

    // 2. Get account
    let fetched = db.get_account("acc_claude_1").unwrap().unwrap();
    assert_eq!(fetched.id, account.id);
    assert_eq!(fetched.provider, account.provider);
    assert_eq!(fetched.identity, account.identity);
    assert_eq!(fetched.alias, account.alias);
    assert_eq!(fetched.auth_method, account.auth_method);
    assert_eq!(fetched.plan_raw, account.plan_raw);
    assert_eq!(fetched.plan_normalized, account.plan_normalized);
    assert_eq!(fetched.active, account.active);
    assert_eq!(fetched.health, account.health);
    assert_eq!(fetched.credential_reference, account.credential_reference);

    // 3. List accounts
    let accounts = db.list_accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].id, "acc_claude_1");

    // 4. Update account
    let mut updated_account = account.clone();
    updated_account.alias = Some("Updated Claude".into());
    updated_account.health = AccountHealth::UsageLimited;
    db.save_account(&updated_account).unwrap();

    let refetched = db.get_account("acc_claude_1").unwrap().unwrap();
    assert_eq!(refetched.alias, Some("Updated Claude".into()));
    assert_eq!(refetched.health, AccountHealth::UsageLimited);

    // 5. Delete account
    db.delete_account("acc_claude_1").unwrap();
    let after_delete = db.get_account("acc_claude_1").unwrap();
    assert!(after_delete.is_none());
    assert_eq!(db.list_accounts().unwrap().len(), 0);
}

#[test]
fn test_database_snapshot_lifecycle() {
    let db = DatabaseRepository::open_in_memory().unwrap();
    db.migrate().unwrap();

    let account = Account {
        id: "acc_codex_1".into(),
        provider: "codex".into(),
        identity: "user@openai.com".into(),
        alias: None,
        auth_method: AuthMethod::ApiKey,
        plan_raw: Some("plus".into()),
        plan_normalized: Some(NormalizedPlan::OpenAiPlus),
        active: Some(true),
        health: AccountHealth::Ready,
        credential_reference: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_refresh_at: None,
    };
    db.save_account(&account).unwrap();

    let mut provider_meta = BTreeMap::new();
    provider_meta.insert("tier".to_string(), "plus".to_string());

    let window1 = UsageWindow {
        id: "w_5h".into(),
        raw_provider_id: Some("session_5h".into()),
        label: "5-Hour Session".into(),
        scope: UsageScope::AllAccountUsage,
        window_type: WindowType::Rolling,
        enforcement: EnforcementType::Hard,
        duration_seconds: Some(18000),
        used_percent: Some(45.5),
        remaining_percent: Some(54.5),
        consumed_units: Some(455.0),
        limit_units: Some(1000.0),
        unit: Some(UsageUnit::Tokens),
        reset_at: Some(Utc::now()),
        observed_at: Utc::now(),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        provider_metadata: provider_meta.clone(),
    };

    let window2 = UsageWindow {
        id: "w_weekly".into(),
        raw_provider_id: None,
        label: "Weekly Limit".into(),
        scope: UsageScope::AllModels,
        window_type: WindowType::Fixed,
        enforcement: EnforcementType::Soft,
        duration_seconds: Some(604800),
        used_percent: Some(20.0),
        remaining_percent: Some(80.0),
        consumed_units: None,
        limit_units: None,
        unit: Some(UsageUnit::Percent),
        reset_at: None,
        observed_at: Utc::now(),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        provider_metadata: BTreeMap::new(),
    };

    let snapshot = UsageSnapshot {
        account_id: "acc_codex_1".into(),
        provider: "codex".into(),
        plan_raw: Some("plus".into()),
        observed_at: Utc::now(),
        windows: vec![window1, window2],
        limiting_window_id: Some("w_5h".into()),
        effective_used_percent: Some(45.5),
        hard_limit_reached: false,
        availability: AccountAvailability::AvailableIncluded,
        additional_usage: Some(AdditionalUsage {
            enabled: true,
            exhausted: Some(false),
            balance: Some(Money {
                amount: 15.50,
                currency: "USD".into(),
            }),
            spent: None,
            spending_limit: None,
            used_percent: None,
            shared_scope: None,
        }),
        source: ObservationSource::Direct,
        confidence: ObservationConfidence::High,
        raw_metadata: provider_meta,
    };

    db.save_snapshot(&snapshot).unwrap();

    let fetched = db.get_latest_snapshot("acc_codex_1").unwrap().unwrap();
    assert_eq!(fetched.account_id, "acc_codex_1");
    assert_eq!(fetched.provider, "codex");
    assert_eq!(fetched.limiting_window_id.as_deref(), Some("w_5h"));
    assert_eq!(fetched.effective_used_percent, Some(45.5));
    assert_eq!(fetched.hard_limit_reached, false);
    assert_eq!(fetched.availability, AccountAvailability::AvailableIncluded);
    assert_eq!(fetched.windows.len(), 2);

    let w1_fetched = fetched.windows.iter().find(|w| w.id == "w_5h").unwrap();
    assert_eq!(w1_fetched.label, "5-Hour Session");
    assert_eq!(w1_fetched.used_percent, Some(45.5));
    assert_eq!(w1_fetched.unit, Some(UsageUnit::Tokens));
    assert_eq!(w1_fetched.provider_metadata.get("tier").map(String::as_str), Some("plus"));

    let add_usage = fetched.additional_usage.unwrap();
    assert_eq!(add_usage.enabled, true);
    assert_eq!(add_usage.balance.unwrap().amount, 15.50);
}

#[test]
fn test_keyring_store_interface() {
    // KeyringStore might fail if OS secret service is unavailable in CI/headless,
    // so we verify the functions return a Result.
    let res = KeyringStore::store_secret("limitlane_test_svc", "test_user", "secret123");
    if res.is_ok() {
        let read_res = KeyringStore::get_secret("limitlane_test_svc", "test_user");
        assert_eq!(read_res.unwrap(), Some("secret123".to_string()));
        let del_res = KeyringStore::delete_secret("limitlane_test_svc", "test_user");
        assert!(del_res.is_ok());
    }
}
