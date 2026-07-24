use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::domain::evaluator::evaluate_limiting_window;
use crate::domain::types::{AccountHealth, UsageSnapshot};
use crate::error::LimitLaneError;
use crate::provider::adapter::ProviderAdapter;
use crate::storage::db::DatabaseRepository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataFreshness {
    Fresh,
    Aging,
    Stale,
    VeryStale,
}

pub fn classify_freshness(observed_at: DateTime<Utc>) -> DataFreshness {
    classify_freshness_at(observed_at, Utc::now())
}

pub fn classify_freshness_at(observed_at: DateTime<Utc>, at: DateTime<Utc>) -> DataFreshness {
    let age = at.signed_duration_since(observed_at);
    if age <= Duration::seconds(120) {
        DataFreshness::Fresh
    } else if age <= Duration::seconds(600) {
        DataFreshness::Aging
    } else if age <= Duration::seconds(3600) {
        DataFreshness::Stale
    } else {
        DataFreshness::VeryStale
    }
}

#[derive(Debug, Clone)]
pub struct RefreshConfig {
    pub interval: StdDuration,
    pub timeout: StdDuration,
    pub max_concurrent: usize,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            interval: StdDuration::from_secs(60),
            timeout: StdDuration::from_secs(15),
            max_concurrent: 4,
        }
    }
}

#[derive(Clone)]
pub struct RefreshService {
    repo: Arc<DatabaseRepository>,
    providers: HashMap<String, Arc<dyn ProviderAdapter>>,
    config: RefreshConfig,
    semaphore: Arc<Semaphore>,
}

impl RefreshService {
    pub fn new(
        repo: Arc<DatabaseRepository>,
        providers: HashMap<String, Arc<dyn ProviderAdapter>>,
        config: RefreshConfig,
    ) -> Self {
        let max_concurrent = config.max_concurrent;
        Self {
            repo,
            providers,
            config,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub fn register_provider(
        &mut self,
        provider_id: impl Into<String>,
        adapter: Arc<dyn ProviderAdapter>,
    ) {
        self.providers.insert(provider_id.into(), adapter);
    }

    pub async fn refresh_account(&self, account_id: &str) -> Result<UsageSnapshot, LimitLaneError> {
        let mut account = match self.repo.get_account(account_id)? {
            Some(acc) => acc,
            None => {
                return Err(LimitLaneError::Configuration(format!(
                    "Account not found: {account_id}"
                )));
            }
        };

        let adapter = match self.providers.get(&account.provider) {
            Some(adapter) => adapter.clone(),
            None => {
                return Err(LimitLaneError::Provider {
                    provider: account.provider.clone(),
                    message: format!("No adapter registered for provider '{}'", account.provider),
                });
            }
        };

        let fetch_res = timeout(self.config.timeout, adapter.fetch_usage(&account)).await;

        match fetch_res {
            Ok(Ok(mut snapshot)) => {
                evaluate_limiting_window(&mut snapshot);
                self.repo.save_snapshot(&snapshot)?;

                account.last_refresh_at = Some(snapshot.observed_at);
                account.health = AccountHealth::Ready;
                account.updated_at = Utc::now();
                self.repo.save_account(&account)?;

                Ok(snapshot)
            }
            Ok(Err(err)) => {
                let raw_msg = err.to_string();
                let redacted_msg = redact_sensitive_info(&raw_msg);
                let _ = self
                    .repo
                    .save_refresh_error(account_id, &account.provider, &redacted_msg);

                account.health = AccountHealth::TemporarilyUnreachable;
                account.updated_at = Utc::now();
                let _ = self.repo.save_account(&account);

                Err(err)
            }
            Err(_) => {
                let err_msg = format!(
                    "Refresh timed out after {}s for account {}",
                    self.config.timeout.as_secs(),
                    account_id
                );
                let err = LimitLaneError::Network {
                    url: account.provider.clone(),
                    message: err_msg.clone(),
                };
                let redacted_msg = redact_sensitive_info(&err_msg);
                let _ = self
                    .repo
                    .save_refresh_error(account_id, &account.provider, &redacted_msg);

                account.health = AccountHealth::TemporarilyUnreachable;
                account.updated_at = Utc::now();
                let _ = self.repo.save_account(&account);

                Err(err)
            }
        }
    }

    pub async fn refresh_all_accounts(&self) -> Vec<Result<UsageSnapshot, LimitLaneError>> {
        let accounts = match self.repo.list_accounts() {
            Ok(accs) => accs,
            Err(e) => return vec![Err(e)],
        };

        let mut handles = Vec::new();
        for account in accounts {
            let service = self.clone();
            let handle = tokio::spawn(async move {
                let _permit = service.semaphore.acquire().await.unwrap();
                service.refresh_account(&account.id).await
            });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(res) => results.push(res),
                Err(e) => results.push(Err(LimitLaneError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))),
            }
        }

        results
    }
}

fn redact_sensitive_info(msg: &str) -> String {
    let mut result = msg.to_string();
    while let Some(start) = result.find("sk-") {
        let rest = &result[start..];
        let len = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .count();
        if len > 3 {
            result.replace_range(start..start + len, "[REDACTED]");
        } else {
            break;
        }
    }
    result
}
