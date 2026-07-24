# LimitLane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build LimitLane, a native terminal user interface (TUI) and CLI application in Rust for monitoring usage limits across Anthropic Claude Code and OpenAI Codex accounts.

**Architecture:** A modular Rust application with pure domain types in `domain`, dynamic provider adapters in `provider`, SQLite + keyring persistence in `storage`, async Tokio refresh scheduling in `service`, Clap CLI commands in `cli`, and interactive Ratatui dashboard in `tui`.

**Tech Stack:** Rust 2021 Edition, Tokio (async runtime), Ratatui + Crossterm (TUI), Clap (CLI), Reqwest (HTTP), Serde / serde_json (serialization), Rusqlite (SQLite database), Keyring (OS secret storage), Tracing (redacted logging), Chrono (datetime), Insta (snapshot testing).

## Global Constraints

- Language: Full Rust (2021 edition).
- No web tech: No Tauri, React, Node.js, Bun, browser, webview, desktop GUI, or mobile UI.
- Terminal minimum target: 80 columns × 24 rows.
- No fabricated precision: Missing percentage or reset time must be represented as `None` (`Unknown` in UI / `null` in JSON), never zero.
- Multi-window: Account must support multiple concurrent `UsageWindow` instances.
- Hardcoded cycles forbidden: Never assume 5-hour sessions or weekly limits apply universally to Codex.
- Local-first & secure: Credentials stored in OS keyring; snapshots in local SQLite; zero remote telemetry; logs redact secrets.

---

### Task 1: Project Scaffolding & Error Definitions

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/error.rs`

**Interfaces:**
- Produces: `LimitLaneError` enum deriving `thiserror::Error` for domain, authentication, network, provider, storage, and parser errors.

- [ ] **Step 1: Write Cargo.toml with all required dependencies**

```toml
[package]
name = "limitlane"
version = "0.1.0"
edition = "2021"
authors = ["LimitLane Developers"]
description = "Native terminal dashboard for monitoring AI coding-provider usage limits"

[dependencies]
tokio = { version = "1.38", features = ["full"] }
ratatui = "0.26"
crossterm = "0.27"
clap = { version = "4.5", features = ["derive", "env"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
rusqlite = { version = "0.31", features = ["bundled"] }
keyring = "2.3"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1.0"
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
async-trait = "0.1"
directories = "5.0"

[dev-dependencies]
insta = { version = "1.38", features = ["yaml"] }
wiremock = "0.6"
tempfile = "3.10"
```

- [ ] **Step 2: Create src/error.rs error definitions**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LimitLaneError {
    #[error("Authentication error on provider '{provider}' for account '{account_id}': {message}")]
    Authentication { provider: String, account_id: String, message: String },

    #[error("Credential store error: {0}")]
    CredentialStore(String),

    #[error("Network error accessing '{url}': {message}")]
    Network { url: String, message: String },

    #[error("Provider '{provider}' error: {message}")]
    Provider { provider: String, message: String },

    #[error("Failed to parse provider response from '{provider}': {message}")]
    Parsing { provider: String, message: String },

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("IO error: {0}")]
    Io(#[from] std.io::Error),
}
```

- [ ] **Step 3: Create src/lib.rs and src/main.rs**

```rust
// src/lib.rs
pub mod error;
pub mod domain;
pub mod provider;
pub mod storage;
pub mod service;
pub mod doctor;
pub mod cli;
pub mod tui;
```

```rust
// src/main.rs
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("LimitLane v0.1.0");
    Ok(())
}
```

- [ ] **Step 4: Verify build and test compilation**

Run: `cargo check`
Expected: PASS with 0 errors.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/
git commit -m "feat: initialize project structure and error definitions"
```

---

### Task 2: Pure Domain Types & Limiting-Window Evaluator

**Files:**
- Create: `src/domain/mod.rs`
- Create: `src/domain/types.rs`
- Create: `src/domain/evaluator.rs`
- Test: `tests/evaluator_test.rs`

**Interfaces:**
- Produces: `Account`, `UsageSnapshot`, `UsageWindow`, `AdditionalUsage`, `AccountAvailability`, `AccountHealth`, `NormalizedPlan`.
- Produces: `fn evaluate_limiting_window(snapshot: &mut UsageSnapshot)`

- [ ] **Step 1: Write test for limiting window evaluator**

```rust
// tests/evaluator_test.rs
use limitlane::domain::types::*;
use limitlane::domain::evaluator::evaluate_limiting_window;
use chrono::Utc;

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
        provider_metadata: std::collections::BTreeMap::new(),
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
        raw_metadata: std::collections::BTreeMap::new(),
    };

    evaluate_limiting_window(&mut snapshot);

    assert_eq!(snapshot.limiting_window_id.as_deref(), Some("w2"));
    assert_eq!(snapshot.effective_used_percent, Some(85.0));
    assert_eq!(snapshot.availability, AccountAvailability::AvailableIncluded);
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
        provider_metadata: std::collections::BTreeMap::new(),
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
        raw_metadata: std::collections::BTreeMap::new(),
    };

    evaluate_limiting_window(&mut snapshot);

    assert_eq!(snapshot.hard_limit_reached, true);
    assert_eq!(snapshot.availability, AccountAvailability::AvailablePaidOverflow);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test evaluator_test`
Expected: FAIL due to missing domain types.

- [ ] **Step 3: Create src/domain/types.rs**

Write full data structures (`Account`, `UsageSnapshot`, `UsageWindow`, `UsageScope`, `WindowType`, `EnforcementType`, `UsageUnit`, `AdditionalUsage`, `AccountAvailability`, `AccountHealth`, `NormalizedPlan`, `ObservationSource`, `ObservationConfidence`).

- [ ] **Step 4: Create src/domain/evaluator.rs**

Implement `evaluate_limiting_window(&mut snapshot)`:
1. Filter windows where `enforcement == EnforcementType::Hard`.
2. Find window with maximum `used_percent`.
3. Set `limiting_window_id` and `effective_used_percent`.
4. If max percentage >= 100.0 or any window reports rejection, set `hard_limit_reached = true`.
5. Update `availability` status (`AvailablePaidOverflow` vs `Limited` vs `AvailableIncluded`).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test evaluator_test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/domain/ tests/evaluator_test.rs
git commit -m "feat: add domain types and limiting window evaluator algorithm"
```

---

### Task 3: Storage Layer (SQLite Repository & Keyring Secret Manager)

**Files:**
- Create: `src/storage/mod.rs`
- Create: `src/storage/db.rs`
- Create: `src/storage/keyring.rs`
- Test: `tests/storage_test.rs`

**Interfaces:**
- Produces: `struct DatabaseRepository` with schema migrations, account CRUD, snapshot insertion & querying.
- Produces: `struct KeyringStore` for storing/retrieving API keys & OAuth tokens in OS Keyring.

- [ ] **Step 1: Write test for storage repository**

```rust
// tests/storage_test.rs
use limitlane::storage::db::DatabaseRepository;
use limitlane::domain::types::*;
use chrono::Utc;
use tempfile::NamedTempFile;

#[test]
fn test_database_account_and_snapshot_lifecycle() {
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

    db.save_account(&account).unwrap();
    let fetched = db.get_account("acc_claude_1").unwrap().unwrap();
    assert_eq!(fetched.identity, "dev@example.com");

    let accounts = db.list_accounts().unwrap();
    assert_eq!(accounts.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test storage_test`
Expected: FAIL due to missing `DatabaseRepository`.

- [ ] **Step 3: Implement DatabaseRepository in src/storage/db.rs**

Create SQLite tables:
- `accounts` table.
- `snapshots` table.
- `windows` table.
- `refresh_errors` table.

Implement methods: `open`, `migrate`, `save_account`, `get_account`, `list_accounts`, `delete_account`, `save_snapshot`, `get_latest_snapshot`.

- [ ] **Step 4: Implement KeyringStore in src/storage/keyring.rs**

```rust
use crate::error::LimitLaneError;
use keyring::Entry;

pub struct KeyringStore;

impl KeyringStore {
    pub fn store_secret(service: &str, username: &str, secret: &str) -> Result<(), LimitLaneError> {
        let entry = Entry::new(service, username)
            .map_err(|e| LimitLaneError::CredentialStore(e.to_string()))?;
        entry.set_password(secret)
            .map_err(|e| LimitLaneError::CredentialStore(e.to_string()))?;
        Ok(())
    }

    pub fn get_secret(service: &str, username: &str) -> Result<Option<String>, LimitLaneError> {
        let entry = Entry::new(service, username)
            .map_err(|e| LimitLaneError::CredentialStore(e.to_string()))?;
        match entry.get_password() {
            Ok(pwd) => Ok(Some(pwd)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(LimitLaneError::CredentialStore(e.to_string())),
        }
    }

    pub fn delete_secret(service: &str, username: &str) -> Result<(), LimitLaneError> {
        let entry = Entry::new(service, username)
            .map_err(|e| LimitLaneError::CredentialStore(e.to_string()))?;
        match entry.delete_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(LimitLaneError::CredentialStore(e.to_string())),
        }
    }
}
```

- [ ] **Step 5: Run storage test to verify it passes**

Run: `cargo test --test storage_test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/storage/ tests/storage_test.rs
git commit -m "feat: add SQLite database repository and keyring secret store"
```

---

### Task 4: Provider Adapter Base Trait & Parser Fixtures

**Files:**
- Create: `src/provider/mod.rs`
- Create: `src/provider/adapter.rs`
- Create: `tests/fixtures/claude_pro.json`
- Create: `tests/fixtures/codex_plus.json`

**Interfaces:**
- Produces: `async_trait ProviderAdapter` interface.

- [ ] **Step 1: Create src/provider/adapter.rs**

```rust
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
}
```

- [ ] **Step 2: Add fixture JSON files for tests**

Create `tests/fixtures/claude_pro.json` and `tests/fixtures/codex_plus.json` representing sample provider usage API responses.

- [ ] **Step 3: Commit**

```bash
git add src/provider/ tests/fixtures/
git commit -m "feat: define ProviderAdapter trait and test fixtures"
```

---

### Task 5: Anthropic Claude Provider Adapter

**Files:**
- Create: `src/provider/claude.rs`
- Test: `tests/claude_parser_test.rs`

**Interfaces:**
- Produces: `ClaudeAdapter` implementing `ProviderAdapter`. Dynamic parsing for 5-hour session, weekly all-model, weekly Sonnet, and unknown model windows.

- [ ] **Step 1: Write test for Claude usage parsing**

```rust
// tests/claude_parser_test.rs
use limitlane::provider::claude::ClaudeAdapter;
use limitlane::domain::types::*;

#[tokio::test]
async fn test_parse_claude_usage_fixture() {
    let fixture = include_str!("fixtures/claude_pro.json");
    let adapter = ClaudeAdapter::new_mock();
    let snapshot = adapter.parse_usage_json("acc_claude_1", fixture).unwrap();

    assert_eq!(snapshot.provider, "claude");
    assert!(snapshot.windows.len() >= 2);
    assert_eq!(snapshot.windows[0].label, "5-hour session");
}
```

- [ ] **Step 2: Implement ClaudeAdapter in src/provider/claude.rs**

Implement parsing logic converting Claude usage JSON (session limits, weekly limits, extra credits) into `UsageSnapshot` and `UsageWindow`. Unknown fields preserved in `provider_metadata`.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --test claude_parser_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/provider/claude.rs tests/claude_parser_test.rs
git commit -m "feat: add Claude Code provider adapter and dynamic window parser"
```

---

### Task 6: OpenAI Codex Provider Adapter

**Files:**
- Create: `src/provider/codex.rs`
- Test: `tests/codex_parser_test.rs`

**Interfaces:**
- Produces: `CodexAdapter` implementing `ProviderAdapter`. Dynamic parsing for agentic pool, token usage, dynamic windows, additional credits.

- [ ] **Step 1: Write test for Codex usage parsing**

```rust
// tests/codex_parser_test.rs
use limitlane::provider::codex::CodexAdapter;
use limitlane::domain::types::*;

#[tokio::test]
async fn test_parse_codex_usage_fixture() {
    let fixture = include_str!("fixtures/codex_plus.json");
    let adapter = CodexAdapter::new_mock();
    let snapshot = adapter.parse_usage_json("acc_codex_1", fixture).unwrap();

    assert_eq!(snapshot.provider, "codex");
    assert!(snapshot.additional_usage.is_some());
}
```

- [ ] **Step 2: Implement CodexAdapter in src/provider/codex.rs**

Implement parsing logic converting Codex API/client usage responses into `UsageSnapshot`, dynamic `UsageWindow` (without assuming 5h cycle), and `AdditionalUsage`.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --test codex_parser_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/provider/codex.rs tests/codex_parser_test.rs
git commit -m "feat: add OpenAI Codex provider adapter and token/credit parser"
```

---

### Task 7: Refresh Scheduler & Service Layer

**Files:**
- Create: `src/service/mod.rs`
- Create: `src/service/refresh.rs`
- Test: `tests/refresh_service_test.rs`

**Interfaces:**
- Produces: `RefreshService` managing bounded concurrent refreshes (max 4, 15s timeout per account, 60s interval), handling partial failures without invalidating cached snapshots.

- [ ] **Step 1: Implement RefreshService in src/service/refresh.rs**

Uses Tokio `Semaphore` for concurrency gating and exponential backoff retry. Saves fresh snapshots to SQLite upon completion.

- [ ] **Step 2: Write integration test for RefreshService**

Verify single account failure does not block other accounts and last valid snapshot remains queryable.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --test refresh_service_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/service/ tests/refresh_service_test.rs
git commit -m "feat: add RefreshService with bounded concurrency and snapshot caching"
```

---

### Task 8: Doctor Diagnostic Command

**Files:**
- Create: `src/doctor/mod.rs`
- Create: `src/doctor/checker.rs`
- Test: `tests/doctor_test.rs`

**Interfaces:**
- Produces: `fn run_doctor_checks() -> DiagnosticReport` returning structured results for OS, terminal size, config directory, SQLite DB access, Keyring storage, Codex/Claude installations, network connectivity.

- [ ] **Step 1: Write test for Doctor report serialization**

```rust
// tests/doctor_test.rs
use limitlane::doctor::checker::run_doctor_checks;

#[tokio::test]
async fn test_doctor_output_json() {
    let report = run_doctor_checks().await;
    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("os"));
    assert!(json.contains("keyring"));
}
```

- [ ] **Step 2: Implement run_doctor_checks in src/doctor/checker.rs**

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --test doctor_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/doctor/ tests/doctor_test.rs
git commit -m "feat: implement doctor command diagnostic health checks"
```

---

### Task 9: CLI Interface & Versioned JSON Export

**Files:**
- Create: `src/cli/mod.rs`
- Create: `src/cli/args.rs`
- Create: `src/cli/commands.rs`
- Test: `tests/cli_json_test.rs`

**Interfaces:**
- Produces: Clap CLI parsers for `limitlane`, `limitlane status [--json]`, `limitlane accounts`, `limitlane usage`, `limitlane refresh`, `limitlane doctor`.

- [ ] **Step 1: Write test for CLI status --json schema_version**

```rust
// tests/cli_json_test.rs
use limitlane::cli::commands::format_status_json;

#[test]
fn test_status_json_schema_v1() {
    let json_str = format_status_json(&[]).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["schema_version"], "1");
}
```

- [ ] **Step 2: Implement Clap subcommand hierarchy in src/cli/**

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --test cli_json_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/cli/ tests/cli_json_test.rs
git commit -m "feat: implement Clap CLI commands and versioned JSON output"
```

---

### Task 10: Interactive TUI Dashboard (Ratatui + Crossterm)

**Files:**
- Create: `src/tui/mod.rs`
- Create: `src/tui/app.rs`
- Create: `src/tui/ui.rs`
- Create: `src/tui/events.rs`
- Create: `src/tui/widgets/dashboard.rs`
- Create: `src/tui/widgets/details.rs`
- Create: `src/tui/widgets/credits.rs`
- Create: `src/tui/widgets/accounts.rs`
- Create: `src/tui/widgets/filters.rs`
- Test: `tests/tui_snapshot_test.rs`

**Interfaces:**
- Produces: Ratatui event loop and screens for Dashboard, Account Detail, Credits View, Accounts Management, and Filters.
- Support 80x24 responsive layout with ellipsis truncation. Keybindings (`j/k`, `Enter`, `Esc`, `Tab`, `r/R`, `a`, `d`, `f`, `/`, `?`, `q`).

- [ ] **Step 1: Implement App state & event handling in src/tui/app.rs and events.rs**

- [ ] **Step 2: Implement UI layout rendering in src/tui/ui.rs and widgets/**

Render table columns: Provider, Account, Plan, Limiting Scope, Used %, Reset Countdown.
Handle responsive column hiding when width < 80 cols.

- [ ] **Step 3: Write Ratatui snapshot tests using insta in tests/tui_snapshot_test.rs**

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test tui_snapshot_test`
Expected: PASS

- [ ] **Step 5: Connect main.rs to launch TUI when run without subcommands**

- [ ] **Step 6: Verify full application build & tests**

Run: `cargo test`
Expected: PASS with all unit, integration, and parser tests passing.

- [ ] **Step 7: Commit**

```bash
git add src/tui/ src/main.rs tests/tui_snapshot_test.rs
git commit -m "feat: implement interactive Ratatui dashboard and responsive layout"
```
