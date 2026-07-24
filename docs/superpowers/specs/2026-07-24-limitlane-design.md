# Design Specification: LimitLane

**Date:** 2026-07-24  
**Status:** Approved  
**Target:** Full Rust Native TUI/CLI Terminal Application  
**PRD Source:** `attachments/1784911613339-limitlane-prd-en.md`

---

## 1. Executive Overview

LimitLane is a local-first, native terminal application written in Rust that monitors usage limits across multiple AI coding provider accounts (OpenAI Codex and Anthropic Claude Code) from a unified dashboard.

Key capabilities:
- Multi-account management for OpenAI Codex and Anthropic Claude.
- Automatic account discovery and plan detection.
- Multi-window usage monitoring (session limits, weekly limits, model-specific caps, shared pools).
- Dynamic provider-reported window parsing without assuming hardcoded 5-hour cycles for all providers.
- Limiting window evaluator algorithm to highlight the primary bottleneck.
- Distinguishes included usage, additional credits, and paid overflow status.
- Keyring-backed OS secure credential storage & SQLite local metadata/snapshot storage.
- Interactive TUI (Ratatui + Crossterm) responsive down to 80x24.
- Full CLI interface with versioned JSON outputs (`limitlane status --json`) and diagnostic `limitlane doctor`.

---

## 2. Core Domain Models

### 2.1 Account & Health
```rust
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
```

### 2.2 Dynamic Usage Window & Snapshot
```rust
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

pub enum WindowType {
    Rolling,
    Fixed,
    CreditBalance,
    Spend,
    DemandAdjusted,
    ProviderDefined,
    Unknown,
}

pub enum EnforcementType {
    Hard,
    Soft,
    PaidOverflow,
    Informational,
    Unknown,
}

pub enum UsageUnit {
    Percent,
    Credits,
    Tokens,
    Currency(String),
    Messages,
    Tasks,
    Unknown(String),
}

pub struct AdditionalUsage {
    pub enabled: bool,
    pub exhausted: Option<bool>,
    pub balance: Option<Money>,
    pub spent: Option<Money>,
    pub spending_limit: Option<Money>,
    pub used_percent: Option<f64>,
    pub shared_scope: Option<String>,
}

pub enum AccountAvailability {
    AvailableIncluded,
    AvailablePaidOverflow,
    Limited,
    AuthenticationRequired,
    TemporarilyUnavailable,
    Unknown,
}

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
```

---

## 3. Architecture & Component Modules

```
src/
├── main.rs                 # CLI entry point and TUI launcher
├── cli/                    # Clap CLI command definitions & execution
├── tui/                    # Ratatui application state, rendering, event loop, keybindings
│   ├── app.rs
│   ├── ui.rs
│   ├── widgets/
│   └── events.rs
├── domain/                 # Pure domain types, limiting window evaluator, pressure status
├── provider/               # ProviderAdapter trait, Claude & Codex adapters, mock/fixture parsers
│   ├── adapter.rs
│   ├── claude.rs
│   └── codex.rs
├── storage/                # SQLite metadata & snapshot repository (rusqlite), Keyring secret store
│   ├── db.rs
│   └── keyring.rs
├── service/                # Async refresh scheduler, aggregator, retry backoff
└── doctor/                 # System diagnostics & health check logic
```

### 3.1 Limiting-Window Evaluator Logic
1. Collect hard-enforced windows (`EnforcementType::Hard`).
2. Set `hard_limit_reached` if provider explicitly reports request rejection or window at 100%.
3. Select window with highest known `used_percent`.
4. If no percentages, fallback to explicit provider window status.
5. If included usage exhausted (`used_percent >= 100%`) and additional credits available, set availability to `AvailablePaidOverflow`.

### 3.2 Provider Adapter Trait
```rust
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn provider_id(&self) -> ProviderId;
    async fn discover_accounts(&self) -> Result<Vec<DiscoveredAccount>, ProviderError>;
    async fn detect_active_account(&self) -> Result<Option<AccountIdentity>, ProviderError>;
    async fn fetch_account_metadata(&self, account: &Account) -> Result<AccountMetadata, ProviderError>;
    async fn fetch_usage(&self, account: &Account) -> Result<UsageSnapshot, ProviderError>;
    async fn validate_auth(&self, account: &Account) -> Result<AccountHealth, ProviderError>;
}
```

---

## 4. Storage & Security

- **Configuration:** Platform directories (`directories` crate) -> `~/.config/limitlane/config.toml` (Linux), `~/Library/Application Support/limitlane/config.toml` (macOS), `%APPDATA%\limitlane\config.toml` (Windows).
- **Metadata Database:** SQLite (`rusqlite` via blocking task pool) storing accounts, raw plan strings, snapshots, windows, refresh errors, and compatibility flags.
- **Secure Credentials:** `keyring` crate interfacing with OS Keychain / Secret Service / Credential Manager. Plaintext secrets are strictly forbidden.
- **Log Security:** `tracing` logs with token/secret redaction. panic hook ensures terminal restoration before output.

---

## 5. CLI & TUI Design

### 5.1 CLI Commands
- `limitlane` -> Launches interactive TUI.
- `limitlane status [--json]` -> Quick summary of accounts and limiting windows.
- `limitlane accounts [add|remove|reauth|list] [--json]` -> Manage registered provider accounts.
- `limitlane usage [--account <ID>] [--provider <P>] [--json]` -> Full window breakdown.
- `limitlane refresh [--account <ID>] [--provider <P>]` -> Trigger usage snapshot update.
- `limitlane doctor [--json]` -> System check (OS, terminal, keyring, DB, connectivity, client installation).

### 5.2 TUI Navigation & Views
- **Dashboard View:** Compact account list showing provider, alias, identity, plan, active status, limiting scope, percentage bar, and reset countdown.
- **Detail View:** Full breakdown of all usage windows, raw metadata, additional credit balances, data freshness, and confidence.
- **Credits View:** Dedicated display for paid overflow, shared pools, and credit consumption.
- **Accounts & Filter View:** Filter by provider, plan, pressure, health, stale state, or paid overflow.

---

## 6. Verification & Testing Strategy

1. **Unit Tests:**
   - Limiting window selection algorithm & paid overflow evaluation.
   - Claude Pro / Max 5x / Max 20x parser tests with redacted fixtures.
   - Codex token-based & shared pool usage parser tests with redacted fixtures.
   - Snapshot fallback on network failure.
2. **Integration Tests:**
   - SQLite migrations and snapshot persistence.
   - Keyring mock storage validation.
   - CLI JSON output schema validation (`schema_version: "1"`).
3. **TUI Tests:**
   - Ratatui terminal buffer snapshot tests (`insta`) across 80x24 and wide views.
