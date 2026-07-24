use async_trait::async_trait;
use chrono::{Duration, Utc};
use limitlane::domain::types::*;
use limitlane::error::LimitLaneError;
use limitlane::provider::adapter::ProviderAdapter;
use limitlane::service::refresh::{classify_freshness, classify_freshness_at, DataFreshness, RefreshConfig, RefreshService};
use limitlane::storage::db::DatabaseRepository;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::sleep;

struct MockAdapter {
    provider_id: String,
    delay: Option<std::time::Duration>,
    fail_account_id: Option<String>,
    active_count: Arc<AtomicUsize>,
    max_observed_concurrent: Arc<AtomicUsize>,
}

#[async_trait]
impl ProviderAdapter for MockAdapter {
    fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    async fn discover_accounts(&self) -> Result<Vec<DiscoveredAccount>, LimitLaneError> {
        Ok(vec![])
    }

    async fn detect_active_account(&self) -> Result<Option<AccountIdentity>, LimitLaneError> {
        Ok(None)
    }

    async fn fetch_account_metadata(&self, _account: &Account) -> Result<AccountMetadata, LimitLaneError> {
        Ok(AccountMetadata {
            plan_raw: Some("pro".into()),
            plan_normalized: Some(NormalizedPlan::ClaudePro),
            metadata: Default::default(),
        })
    }

    async fn fetch_usage(&self, account: &Account) -> Result<UsageSnapshot, LimitLaneError> {
        let current = self.active_count.fetch_add(1, Ordering::SeqCst) + 1;
        let mut max = self.max_observed_concurrent.load(Ordering::SeqCst);
        while current > max {
            match self.max_observed_concurrent.compare_exchange_weak(
                max,
                current,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => max = actual,
            }
        }

        if let Some(delay) = self.delay {
            sleep(delay).await;
        }

        self.active_count.fetch_sub(1, Ordering::SeqCst);

        if let Some(ref fail_id) = self.fail_account_id {
            if &account.id == fail_id {
                return Err(LimitLaneError::Network {
                    url: "https://api.mock.com".into(),
                    message: "Connection failed with token sk-secret123".into(),
                });
            }
        }

        Ok(UsageSnapshot {
            account_id: account.id.clone(),
            provider: self.provider_id.clone(),
            plan_raw: Some("pro".into()),
            observed_at: Utc::now(),
            windows: vec![],
            limiting_window_id: None,
            effective_used_percent: Some(25.0),
            hard_limit_reached: false,
            availability: AccountAvailability::AvailableIncluded,
            additional_usage: None,
            source: ObservationSource::Direct,
            confidence: ObservationConfidence::High,
            raw_metadata: Default::default(),
        })
    }

    async fn validate_auth(&self, _account: &Account) -> Result<AccountHealth, LimitLaneError> {
        Ok(AccountHealth::Ready)
    }
}

#[test]
fn test_data_freshness_classification() {
    let now = Utc::now();

    // Fresh <= 2m
    assert_eq!(classify_freshness(Utc::now() - Duration::seconds(30)), DataFreshness::Fresh);
    assert_eq!(classify_freshness_at(now - Duration::seconds(30), now), DataFreshness::Fresh);
    assert_eq!(classify_freshness_at(now - Duration::seconds(120), now), DataFreshness::Fresh);

    // Aging > 2m and <= 10m
    assert_eq!(classify_freshness_at(now - Duration::seconds(121), now), DataFreshness::Aging);
    assert_eq!(classify_freshness_at(now - Duration::seconds(600), now), DataFreshness::Aging);

    // Stale > 10m and <= 1h
    assert_eq!(classify_freshness_at(now - Duration::seconds(601), now), DataFreshness::Stale);
    assert_eq!(classify_freshness_at(now - Duration::seconds(3600), now), DataFreshness::Stale);

    // VeryStale > 1h
    assert_eq!(classify_freshness_at(now - Duration::seconds(3601), now), DataFreshness::VeryStale);
    assert_eq!(classify_freshness_at(now - Duration::seconds(7200), now), DataFreshness::VeryStale);
}

#[tokio::test]
async fn test_refresh_single_account() {
    let repo = Arc::new(DatabaseRepository::open_in_memory().unwrap());
    repo.migrate().unwrap();

    let account = Account {
        id: "acc_single".into(),
        provider: "claude".into(),
        identity: "user@example.com".into(),
        alias: None,
        auth_method: AuthMethod::OAuth,
        plan_raw: Some("pro".into()),
        plan_normalized: Some(NormalizedPlan::ClaudePro),
        active: Some(true),
        health: AccountHealth::Ready,
        credential_reference: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_refresh_at: None,
    };
    repo.save_account(&account).unwrap();

    let active_count = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));

    let mock_adapter = Arc::new(MockAdapter {
        provider_id: "claude".into(),
        delay: None,
        fail_account_id: None,
        active_count,
        max_observed_concurrent: max_concurrent,
    });

    let mut providers: HashMap<String, Arc<dyn ProviderAdapter>> = HashMap::new();
    providers.insert("claude".into(), mock_adapter);

    let service = RefreshService::new(repo.clone(), providers, RefreshConfig::default());

    let result = service.refresh_account("acc_single").await;
    assert!(result.is_ok());

    let snapshot = result.unwrap();
    assert_eq!(snapshot.account_id, "acc_single");

    // Check saved in DB
    let db_snapshot = repo.get_latest_snapshot("acc_single").unwrap();
    assert!(db_snapshot.is_some());

    // Check account last_refresh_at updated
    let updated_acc = repo.get_account("acc_single").unwrap().unwrap();
    assert!(updated_acc.last_refresh_at.is_some());
}

#[tokio::test]
async fn test_max_concurrent_refreshes() {
    let repo = Arc::new(DatabaseRepository::open_in_memory().unwrap());
    repo.migrate().unwrap();

    for i in 0..10 {
        let account = Account {
            id: format!("acc_{i}"),
            provider: "claude".into(),
            identity: format!("user{i}@example.com"),
            alias: None,
            auth_method: AuthMethod::OAuth,
            plan_raw: Some("pro".into()),
            plan_normalized: Some(NormalizedPlan::ClaudePro),
            active: Some(true),
            health: AccountHealth::Ready,
            credential_reference: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_refresh_at: None,
        };
        repo.save_account(&account).unwrap();
    }

    let active_count = Arc::new(AtomicUsize::new(0));
    let max_observed = Arc::new(AtomicUsize::new(0));

    let mock_adapter = Arc::new(MockAdapter {
        provider_id: "claude".into(),
        delay: Some(std::time::Duration::from_millis(50)),
        fail_account_id: None,
        active_count,
        max_observed_concurrent: max_observed.clone(),
    });

    let mut providers: HashMap<String, Arc<dyn ProviderAdapter>> = HashMap::new();
    providers.insert("claude".into(), mock_adapter);

    let config = RefreshConfig {
        max_concurrent: 4,
        ..Default::default()
    };

    let service = RefreshService::new(repo.clone(), providers, config);
    let results = service.refresh_all_accounts().await;

    assert_eq!(results.len(), 10);
    for res in results {
        assert!(res.is_ok());
    }

    let observed = max_observed.load(Ordering::SeqCst);
    assert!(observed <= 4, "Max concurrent refreshes was {}, expected <= 4", observed);
}

#[tokio::test]
async fn test_account_refresh_timeout() {
    let repo = Arc::new(DatabaseRepository::open_in_memory().unwrap());
    repo.migrate().unwrap();

    let account = Account {
        id: "acc_timeout".into(),
        provider: "claude".into(),
        identity: "timeout@example.com".into(),
        alias: None,
        auth_method: AuthMethod::OAuth,
        plan_raw: None,
        plan_normalized: None,
        active: Some(true),
        health: AccountHealth::Ready,
        credential_reference: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_refresh_at: None,
    };
    repo.save_account(&account).unwrap();

    let mock_adapter = Arc::new(MockAdapter {
        provider_id: "claude".into(),
        delay: Some(std::time::Duration::from_millis(200)),
        fail_account_id: None,
        active_count: Arc::new(AtomicUsize::new(0)),
        max_observed_concurrent: Arc::new(AtomicUsize::new(0)),
    });

    let mut providers: HashMap<String, Arc<dyn ProviderAdapter>> = HashMap::new();
    providers.insert("claude".into(), mock_adapter);

    let config = RefreshConfig {
        timeout: std::time::Duration::from_millis(50),
        ..Default::default()
    };

    let service = RefreshService::new(repo.clone(), providers, config);
    let result = service.refresh_account("acc_timeout").await;

    assert!(result.is_err());

    // Check error was logged in DB refresh_errors
    let errors = repo.get_refresh_errors("acc_timeout").unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("timed out"));
}

#[tokio::test]
async fn test_partial_failure_handling() {
    let repo = Arc::new(DatabaseRepository::open_in_memory().unwrap());
    repo.migrate().unwrap();

    let acc_ids = vec!["acc_ok1", "acc_fail", "acc_ok2"];
    for id in &acc_ids {
        let account = Account {
            id: (*id).into(),
            provider: "claude".into(),
            identity: format!("{id}@example.com"),
            alias: None,
            auth_method: AuthMethod::OAuth,
            plan_raw: None,
            plan_normalized: None,
            active: Some(true),
            health: AccountHealth::Ready,
            credential_reference: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_refresh_at: None,
        };
        repo.save_account(&account).unwrap();
    }

    let mock_adapter = Arc::new(MockAdapter {
        provider_id: "claude".into(),
        delay: None,
        fail_account_id: Some("acc_fail".into()),
        active_count: Arc::new(AtomicUsize::new(0)),
        max_observed_concurrent: Arc::new(AtomicUsize::new(0)),
    });

    let mut providers: HashMap<String, Arc<dyn ProviderAdapter>> = HashMap::new();
    providers.insert("claude".into(), mock_adapter);

    let service = RefreshService::new(repo.clone(), providers, RefreshConfig::default());
    let results = service.refresh_all_accounts().await;

    assert_eq!(results.len(), 3);

    let ok_count = results.iter().filter(|r| r.is_ok()).count();
    let err_count = results.iter().filter(|r| r.is_err()).count();
    assert_eq!(ok_count, 2);
    assert_eq!(err_count, 1);

    // Verify DB snapshots
    assert!(repo.get_latest_snapshot("acc_ok1").unwrap().is_some());
    assert!(repo.get_latest_snapshot("acc_ok2").unwrap().is_some());
    assert!(repo.get_latest_snapshot("acc_fail").unwrap().is_none());

    // Verify DB error log for acc_fail and redaction
    let errors = repo.get_refresh_errors("acc_fail").unwrap();
    assert_eq!(errors.len(), 1);
    assert!(!errors[0].contains("sk-secret123"));
    assert!(errors[0].contains("[REDACTED]"));
}
