use std::collections::BTreeMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type AccountId = String;
pub type ProviderId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Money {
    pub amount: f64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthMethod {
    OAuth,
    ApiKey,
    SessionToken,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ObservationSource {
    Direct,
    Metadata,
    LocalState,
    CachedSnapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ObservationConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AccountHealth {
    Unchecked,
    Ready,
    RefreshDue,
    Refreshing,
    LoginExpiring,
    ScopeMissing,
    ReauthenticationRequired,
    TemporarilyUnreachable,
    UsageLimited,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NormalizedPlan {
    OpenAiFree,
    OpenAiGo,
    OpenAiPlus,
    OpenAiPro,
    OpenAiBusiness,
    OpenAiEnterprise,
    OpenAiEdu,
    ClaudeFree,
    ClaudePro,
    ClaudeMax5x,
    ClaudeMax20x,
    ClaudeTeam,
    ClaudeEnterprise,
    Api,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub provider: ProviderId,
    pub identity: String,
    pub alias: Option<String>,
    pub auth_method: AuthMethod,
    pub plan_raw: Option<String>,
    pub plan_normalized: Option<NormalizedPlan>,
    pub active: Option<bool>,
    pub health: AccountHealth,
    pub credential_reference: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_refresh_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UsageScope {
    AllAccountUsage,
    SharedAgenticPool,
    AllModels,
    ModelFamily(String),
    Model(String),
    Feature(String),
    ProductSurface(String),
    Credits,
    SpendLimit,
    Workspace,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WindowType {
    Rolling,
    Fixed,
    CreditBalance,
    Spend,
    DemandAdjusted,
    ProviderDefined,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EnforcementType {
    Hard,
    Soft,
    PaidOverflow,
    Informational,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UsageUnit {
    Percent,
    Credits,
    Tokens,
    Currency(String),
    Messages,
    Tasks,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    pub id: String,
    pub raw_provider_id: Option<String>,
    pub label: String,
    pub scope: UsageScope,
    pub window_type: WindowType,
    pub enforcement: EnforcementType,
    pub duration_seconds: Option<u64>,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub consumed_units: Option<f64>,
    pub limit_units: Option<f64>,
    pub unit: Option<UsageUnit>,
    pub reset_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
    pub source: ObservationSource,
    pub confidence: ObservationConfidence,
    pub provider_metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdditionalUsage {
    pub enabled: bool,
    pub exhausted: Option<bool>,
    pub balance: Option<Money>,
    pub spent: Option<Money>,
    pub spending_limit: Option<Money>,
    pub used_percent: Option<f64>,
    pub shared_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AccountAvailability {
    AvailableIncluded,
    AvailablePaidOverflow,
    Limited,
    AuthenticationRequired,
    TemporarilyUnavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub account_id: AccountId,
    pub provider: ProviderId,
    pub plan_raw: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub windows: Vec<UsageWindow>,
    pub limiting_window_id: Option<String>,
    pub effective_used_percent: Option<f64>,
    pub hard_limit_reached: bool,
    pub availability: AccountAvailability,
    pub additional_usage: Option<AdditionalUsage>,
    pub source: ObservationSource,
    pub confidence: ObservationConfidence,
    pub raw_metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredAccount {
    pub provider: ProviderId,
    pub identity: String,
    pub auth_method: AuthMethod,
    pub credential_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountIdentity {
    pub provider: ProviderId,
    pub identity: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountMetadata {
    pub plan_raw: Option<String>,
    pub plan_normalized: Option<NormalizedPlan>,
    pub metadata: BTreeMap<String, String>,
}
