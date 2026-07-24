use crate::domain::types::*;
use crate::error::LimitLaneError;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct DatabaseRepository {
    conn: Mutex<Connection>,
}

impl DatabaseRepository {
    pub fn open(path: &Path) -> Result<Self, LimitLaneError> {
        let conn = Connection::open(path)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self, LimitLaneError> {
        let conn = Connection::open_in_memory()?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn migrate(&self) -> Result<(), LimitLaneError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                identity TEXT NOT NULL,
                alias TEXT,
                auth_method TEXT NOT NULL,
                plan_raw TEXT,
                plan_normalized TEXT,
                active INTEGER,
                health TEXT NOT NULL,
                credential_reference TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_refresh_at TEXT
            );

            CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                plan_raw TEXT,
                observed_at TEXT NOT NULL,
                limiting_window_id TEXT,
                effective_used_percent REAL,
                hard_limit_reached INTEGER NOT NULL,
                availability TEXT NOT NULL,
                additional_usage_json TEXT,
                source TEXT NOT NULL,
                confidence TEXT NOT NULL,
                raw_metadata_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS windows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snapshot_id INTEGER NOT NULL,
                window_id TEXT NOT NULL,
                raw_provider_id TEXT,
                label TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                window_type TEXT NOT NULL,
                enforcement TEXT NOT NULL,
                duration_seconds INTEGER,
                used_percent REAL,
                remaining_percent REAL,
                consumed_units REAL,
                limit_units REAL,
                unit_json TEXT,
                reset_at TEXT,
                observed_at TEXT NOT NULL,
                source TEXT NOT NULL,
                confidence TEXT NOT NULL,
                provider_metadata_json TEXT NOT NULL,
                FOREIGN KEY (snapshot_id) REFERENCES snapshots(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS refresh_errors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                error_message TEXT NOT NULL,
                occurred_at TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    pub fn save_account(&self, account: &Account) -> Result<(), LimitLaneError> {
        let conn = self.conn.lock().unwrap();
        let auth_method = serde_json::to_string(&account.auth_method)
            .map_err(|e| LimitLaneError::Parsing { provider: account.provider.clone(), message: e.to_string() })?;
        let plan_normalized = account
            .plan_normalized
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| LimitLaneError::Parsing { provider: account.provider.clone(), message: e.to_string() })?;
        let health = serde_json::to_string(&account.health)
            .map_err(|e| LimitLaneError::Parsing { provider: account.provider.clone(), message: e.to_string() })?;

        conn.execute(
            "INSERT INTO accounts (
                id, provider, identity, alias, auth_method, plan_raw, plan_normalized,
                active, health, credential_reference, created_at, updated_at, last_refresh_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                identity = excluded.identity,
                alias = excluded.alias,
                auth_method = excluded.auth_method,
                plan_raw = excluded.plan_raw,
                plan_normalized = excluded.plan_normalized,
                active = excluded.active,
                health = excluded.health,
                credential_reference = excluded.credential_reference,
                updated_at = excluded.updated_at,
                last_refresh_at = excluded.last_refresh_at",
            params![
                account.id,
                account.provider,
                account.identity,
                account.alias,
                auth_method,
                account.plan_raw,
                plan_normalized,
                account.active.map(|b| if b { 1 } else { 0 }),
                health,
                account.credential_reference,
                account.created_at.to_rfc3339(),
                account.updated_at.to_rfc3339(),
                account.last_refresh_at.map(|t| t.to_rfc3339()),
            ],
        )?;

        Ok(())
    }

    pub fn get_account(&self, id: &str) -> Result<Option<Account>, LimitLaneError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, provider, identity, alias, auth_method, plan_raw, plan_normalized,
                    active, health, credential_reference, created_at, updated_at, last_refresh_at
             FROM accounts WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let account = parse_account_row(row)?;
            Ok(Some(account))
        } else {
            Ok(None)
        }
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>, LimitLaneError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, provider, identity, alias, auth_method, plan_raw, plan_normalized,
                    active, health, credential_reference, created_at, updated_at, last_refresh_at
             FROM accounts ORDER BY provider, identity",
        )?;

        let rows = stmt.query_map([], parse_account_row)?;
        let mut accounts = Vec::new();
        for r in rows {
            accounts.push(r?);
        }
        Ok(accounts)
    }

    pub fn delete_account(&self, id: &str) -> Result<(), LimitLaneError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn save_snapshot(&self, snapshot: &UsageSnapshot) -> Result<i64, LimitLaneError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let availability_json = serde_json::to_string(&snapshot.availability)
            .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
        let additional_usage_json = snapshot
            .additional_usage
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
        let source_json = serde_json::to_string(&snapshot.source)
            .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
        let confidence_json = serde_json::to_string(&snapshot.confidence)
            .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
        let raw_metadata_json = serde_json::to_string(&snapshot.raw_metadata)
            .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;

        tx.execute(
            "INSERT INTO snapshots (
                account_id, provider, plan_raw, observed_at, limiting_window_id,
                effective_used_percent, hard_limit_reached, availability,
                additional_usage_json, source, confidence, raw_metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                snapshot.account_id,
                snapshot.provider,
                snapshot.plan_raw,
                snapshot.observed_at.to_rfc3339(),
                snapshot.limiting_window_id,
                snapshot.effective_used_percent,
                if snapshot.hard_limit_reached { 1 } else { 0 },
                availability_json,
                additional_usage_json,
                source_json,
                confidence_json,
                raw_metadata_json,
            ],
        )?;

        let snapshot_id = tx.last_insert_rowid();

        for window in &snapshot.windows {
            let scope_json = serde_json::to_string(&window.scope)
                .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
            let window_type_json = serde_json::to_string(&window.window_type)
                .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
            let enforcement_json = serde_json::to_string(&window.enforcement)
                .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
            let unit_json = window
                .unit
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
            let w_source_json = serde_json::to_string(&window.source)
                .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
            let w_confidence_json = serde_json::to_string(&window.confidence)
                .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;
            let provider_metadata_json = serde_json::to_string(&window.provider_metadata)
                .map_err(|e| LimitLaneError::Parsing { provider: snapshot.provider.clone(), message: e.to_string() })?;

            tx.execute(
                "INSERT INTO windows (
                    snapshot_id, window_id, raw_provider_id, label, scope_json,
                    window_type, enforcement, duration_seconds, used_percent,
                    remaining_percent, consumed_units, limit_units, unit_json,
                    reset_at, observed_at, source, confidence, provider_metadata_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    snapshot_id,
                    window.id,
                    window.raw_provider_id,
                    window.label,
                    scope_json,
                    window_type_json,
                    enforcement_json,
                    window.duration_seconds,
                    window.used_percent,
                    window.remaining_percent,
                    window.consumed_units,
                    window.limit_units,
                    unit_json,
                    window.reset_at.map(|t| t.to_rfc3339()),
                    window.observed_at.to_rfc3339(),
                    w_source_json,
                    w_confidence_json,
                    provider_metadata_json,
                ],
            )?;
        }

        tx.commit()?;
        Ok(snapshot_id)
    }

    pub fn get_latest_snapshot(&self, account_id: &str) -> Result<Option<UsageSnapshot>, LimitLaneError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, account_id, provider, plan_raw, observed_at, limiting_window_id,
                    effective_used_percent, hard_limit_reached, availability,
                    additional_usage_json, source, confidence, raw_metadata_json
             FROM snapshots
             WHERE account_id = ?1
             ORDER BY id DESC LIMIT 1",
        )?;

        let mut rows = stmt.query(params![account_id])?;
        if let Some(row) = rows.next()? {
            let snapshot_id: i64 = row.get(0)?;
            let account_id: String = row.get(1)?;
            let provider: String = row.get(2)?;
            let plan_raw: Option<String> = row.get(3)?;
            let observed_at_str: String = row.get(4)?;
            let limiting_window_id: Option<String> = row.get(5)?;
            let effective_used_percent: Option<f64> = row.get(6)?;
            let hard_limit_reached_int: i32 = row.get(7)?;
            let availability_json: String = row.get(8)?;
            let additional_usage_json: Option<String> = row.get(9)?;
            let source_json: String = row.get(10)?;
            let confidence_json: String = row.get(11)?;
            let raw_metadata_json: String = row.get(12)?;

            let observed_at = DateTime::parse_from_rfc3339(&observed_at_str)
                .map_err(|e| LimitLaneError::Parsing { provider: provider.clone(), message: e.to_string() })?
                .with_timezone(&Utc);
            let availability = serde_json::from_str(&availability_json)
                .map_err(|e| LimitLaneError::Parsing { provider: provider.clone(), message: e.to_string() })?;
            let additional_usage = additional_usage_json
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(|e| LimitLaneError::Parsing { provider: provider.clone(), message: e.to_string() })?;
            let source = serde_json::from_str(&source_json)
                .map_err(|e| LimitLaneError::Parsing { provider: provider.clone(), message: e.to_string() })?;
            let confidence = serde_json::from_str(&confidence_json)
                .map_err(|e| LimitLaneError::Parsing { provider: provider.clone(), message: e.to_string() })?;
            let raw_metadata = serde_json::from_str(&raw_metadata_json)
                .map_err(|e| LimitLaneError::Parsing { provider: provider.clone(), message: e.to_string() })?;

            // Fetch windows
            let mut w_stmt = conn.prepare(
                "SELECT window_id, raw_provider_id, label, scope_json, window_type,
                        enforcement, duration_seconds, used_percent, remaining_percent,
                        consumed_units, limit_units, unit_json, reset_at, observed_at,
                        source, confidence, provider_metadata_json
                 FROM windows
                 WHERE snapshot_id = ?1
                 ORDER BY id ASC",
            )?;

            let w_rows = w_stmt.query_map(params![snapshot_id], |w_row| {
                Ok((
                    w_row.get::<_, String>(0)?,
                    w_row.get::<_, Option<String>>(1)?,
                    w_row.get::<_, String>(2)?,
                    w_row.get::<_, String>(3)?,
                    w_row.get::<_, String>(4)?,
                    w_row.get::<_, String>(5)?,
                    w_row.get::<_, Option<u64>>(6)?,
                    w_row.get::<_, Option<f64>>(7)?,
                    w_row.get::<_, Option<f64>>(8)?,
                    w_row.get::<_, Option<f64>>(9)?,
                    w_row.get::<_, Option<f64>>(10)?,
                    w_row.get::<_, Option<String>>(11)?,
                    w_row.get::<_, Option<String>>(12)?,
                    w_row.get::<_, String>(13)?,
                    w_row.get::<_, String>(14)?,
                    w_row.get::<_, String>(15)?,
                    w_row.get::<_, String>(16)?,
                ))
            })?;

            let mut windows = Vec::new();
            for item in w_rows {
                let (
                    window_id,
                    raw_provider_id,
                    label,
                    scope_json,
                    window_type_json,
                    enforcement_json,
                    duration_seconds,
                    used_percent,
                    remaining_percent,
                    consumed_units,
                    limit_units,
                    unit_json,
                    reset_at_str,
                    w_observed_at_str,
                    w_source_json,
                    w_confidence_json,
                    provider_metadata_json,
                ) = item?;

                let scope = serde_json::from_str(&scope_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                let window_type = serde_json::from_str(&window_type_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                let enforcement = serde_json::from_str(&enforcement_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                let unit = unit_json
                    .map(|u| serde_json::from_str(&u))
                    .transpose()
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                let reset_at = reset_at_str
                    .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
                    .transpose()
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                let w_observed_at = DateTime::parse_from_rfc3339(&w_observed_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                let source = serde_json::from_str(&w_source_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                let confidence = serde_json::from_str(&w_confidence_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                let provider_metadata = serde_json::from_str(&provider_metadata_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

                windows.push(UsageWindow {
                    id: window_id,
                    raw_provider_id,
                    label,
                    scope,
                    window_type,
                    enforcement,
                    duration_seconds,
                    used_percent,
                    remaining_percent,
                    consumed_units,
                    limit_units,
                    unit,
                    reset_at,
                    observed_at: w_observed_at,
                    source,
                    confidence,
                    provider_metadata,
                });
            }

            Ok(Some(UsageSnapshot {
                account_id,
                provider,
                plan_raw,
                observed_at,
                windows,
                limiting_window_id,
                effective_used_percent,
                hard_limit_reached: hard_limit_reached_int != 0,
                availability,
                additional_usage,
                source,
                confidence,
                raw_metadata,
            }))
        } else {
            Ok(None)
        }
    }
}

fn parse_account_row(row: &rusqlite::Row) -> Result<Account, rusqlite::Error> {
    let id: String = row.get(0)?;
    let provider: String = row.get(1)?;
    let identity: String = row.get(2)?;
    let alias: Option<String> = row.get(3)?;
    let auth_method_str: String = row.get(4)?;
    let plan_raw: Option<String> = row.get(5)?;
    let plan_normalized_str: Option<String> = row.get(6)?;
    let active_int: Option<i32> = row.get(7)?;
    let health_str: String = row.get(8)?;
    let credential_reference: Option<String> = row.get(9)?;
    let created_at_str: String = row.get(10)?;
    let updated_at_str: String = row.get(11)?;
    let last_refresh_at_str: Option<String> = row.get(12)?;

    let auth_method = serde_json::from_str(&auth_method_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;
    let plan_normalized = plan_normalized_str
        .map(|s| serde_json::from_str(&s))
        .transpose()
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e)))?;
    let health = serde_json::from_str(&health_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e)))?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e)))?;
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(e)))?;
    let last_refresh_at = last_refresh_at_str
        .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
        .transpose()
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(12, rusqlite::types::Type::Text, Box::new(e)))?;

    Ok(Account {
        id,
        provider,
        identity,
        alias,
        auth_method,
        plan_raw,
        plan_normalized,
        active: active_int.map(|i| i != 0),
        health,
        credential_reference,
        created_at,
        updated_at,
        last_refresh_at,
    })
}
