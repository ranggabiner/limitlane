use async_trait::async_trait;
use crate::domain::types::*;
use crate::error::LimitLaneError;

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn provider_id(&self) -> ProviderId;
    async fn discover_accounts(&self) -> Result<Vec<DiscoveredAccount>, LimitLaneError>;
    async fn detect_active_account(&self) -> Result<Option<AccountIdentity>, LimitLaneError>;
    async fn fetch_account_metadata(&self, account: &Account) -> Result<AccountMetadata, LimitLaneError>;
    async fn fetch_usage(&self, account: &Account) -> Result<UsageSnapshot, LimitLaneError>;
    async fn validate_auth(&self, account: &Account) -> Result<AccountHealth, LimitLaneError>;
    fn oauth_login_url(&self) -> Option<String>;
}
