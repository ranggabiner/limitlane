use std::collections::BTreeMap;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::evaluator::evaluate_limiting_window;
use crate::domain::types::*;
use crate::error::LimitLaneError;
use crate::provider::adapter::ProviderAdapter;

#[derive(Debug, Clone)]
pub struct ClaudeAdapter {
    pub base_url: String,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self {
            base_url: "https://api.anthropic.com".to_string(),
        }
    }

    pub fn new_mock() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
        }
    }

    pub fn parse_usage_json(
        &self,
        account_id: &str,
        json_str: &str,
    ) -> Result<UsageSnapshot, LimitLaneError> {
        let value: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
            LimitLaneError::Parsing {
                provider: "claude".to_string(),
                message: format!("Failed to parse JSON: {}", e),
            }
        })?;

        let obj = value.as_object().ok_or_else(|| LimitLaneError::Parsing {
            provider: "claude".to_string(),
            message: "Root JSON is not an object".to_string(),
        })?;

        let plan_raw = obj.get("plan").and_then(|v| v.as_str()).map(|s| s.to_string());
        let now = Utc::now();
        let mut windows = Vec::new();

        if let Some(limits_arr) = obj.get("limits").and_then(|v| v.as_array()) {
            for limit_val in limits_arr {
                if let Some(limit_obj) = limit_val.as_object() {
                    let name = limit_obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown_window");

                    let used_percent = limit_obj.get("used_percent").and_then(|v| v.as_f64());
                    let remaining_percent = used_percent.map(|u| (100.0 - u).max(0.0));

                    let reset_at = limit_obj
                        .get("reset_at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc));

                    let (window_type, duration_seconds, scope, label) = parse_limit_metadata(name);

                    let mut provider_metadata = BTreeMap::new();
                    for (k, v) in limit_obj {
                        if k != "name" && k != "used_percent" && k != "reset_at" {
                            provider_metadata.insert(k.clone(), v.to_string());
                        }
                    }

                    windows.push(UsageWindow {
                        id: name.to_string(),
                        raw_provider_id: Some(name.to_string()),
                        label,
                        scope,
                        window_type,
                        enforcement: EnforcementType::Hard,
                        duration_seconds,
                        used_percent,
                        remaining_percent,
                        consumed_units: None,
                        limit_units: None,
                        unit: Some(UsageUnit::Percent),
                        reset_at,
                        observed_at: now,
                        source: ObservationSource::Direct,
                        confidence: ObservationConfidence::High,
                        provider_metadata,
                    });
                }
            }
        }

        let additional_usage = if let Some(extra) = obj.get("extra_credits").and_then(|v| v.as_object()) {
            let enabled = extra.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let exhausted = extra.get("exhausted").and_then(|v| v.as_bool());
            Some(AdditionalUsage {
                enabled,
                exhausted,
                balance: None,
                spent: None,
                spending_limit: None,
                used_percent: None,
                shared_scope: None,
            })
        } else {
            None
        };

        let mut raw_metadata = BTreeMap::new();
        for (k, v) in obj {
            if k != "account_id" && k != "plan" && k != "limits" && k != "extra_credits" {
                raw_metadata.insert(k.clone(), v.to_string());
            }
        }

        let mut snapshot = UsageSnapshot {
            account_id: account_id.to_string(),
            provider: "claude".to_string(),
            plan_raw,
            observed_at: now,
            windows,
            limiting_window_id: None,
            effective_used_percent: None,
            hard_limit_reached: false,
            availability: AccountAvailability::AvailableIncluded,
            additional_usage,
            source: ObservationSource::Direct,
            confidence: ObservationConfidence::High,
            raw_metadata,
        };

        evaluate_limiting_window(&mut snapshot);

        Ok(snapshot)
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn normalize_claude_plan(plan_str: &str) -> NormalizedPlan {
    match plan_str.to_lowercase().as_str() {
        "free" | "claude_free" => NormalizedPlan::ClaudeFree,
        "pro" | "claude_pro" => NormalizedPlan::ClaudePro,
        "max_5x" | "claude_max_5x" | "max5x" => NormalizedPlan::ClaudeMax5x,
        "max_20x" | "claude_max_20x" | "max20x" => NormalizedPlan::ClaudeMax20x,
        "team" | "claude_team" => NormalizedPlan::ClaudeTeam,
        "enterprise" | "claude_enterprise" => NormalizedPlan::ClaudeEnterprise,
        "api" => NormalizedPlan::Api,
        _ => NormalizedPlan::Unknown,
    }
}

fn parse_limit_metadata(name: &str) -> (WindowType, Option<u64>, UsageScope, String) {
    if name == "5h_session" || name == "session_5h" || name == "5h" || name.contains("5h") {
        (
            WindowType::Rolling,
            Some(18000), // 5 hours = 18000s
            UsageScope::AllAccountUsage,
            "5-hour session".to_string(),
        )
    } else if let Some(suffix) = name.strip_prefix("weekly_") {
        let (scope, label_part) = parse_scope_suffix(suffix);
        (
            WindowType::Fixed,
            Some(604800), // 7 days = 604800s
            scope,
            format!("Weekly {}", label_part),
        )
    } else if let Some(suffix) = name.strip_prefix("monthly_") {
        let (scope, label_part) = parse_scope_suffix(suffix);
        (
            WindowType::Fixed,
            Some(2592000), // 30 days = 2592000s
            scope,
            format!("Monthly {}", label_part),
        )
    } else {
        (
            WindowType::Unknown,
            None,
            UsageScope::AllAccountUsage,
            name.to_string(),
        )
    }
}

fn parse_scope_suffix(suffix: &str) -> (UsageScope, String) {
    match suffix {
        "all_model" | "all_models" | "all" => (UsageScope::AllModels, "All Models".to_string()),
        other => {
            let label = other.replace('_', " ");
            (UsageScope::ModelFamily(other.to_string()), label)
        }
    }
}

#[async_trait]
impl ProviderAdapter for ClaudeAdapter {
    fn provider_id(&self) -> ProviderId {
        "claude".to_string()
    }

    async fn discover_accounts(&self) -> Result<Vec<DiscoveredAccount>, LimitLaneError> {
        Ok(Vec::new())
    }

    async fn detect_active_account(&self) -> Result<Option<AccountIdentity>, LimitLaneError> {
        Ok(None)
    }

    async fn fetch_account_metadata(
        &self,
        account: &Account,
    ) -> Result<AccountMetadata, LimitLaneError> {
        let plan_normalized = account
            .plan_raw
            .as_deref()
            .map(normalize_claude_plan);

        Ok(AccountMetadata {
            plan_raw: account.plan_raw.clone(),
            plan_normalized,
            metadata: BTreeMap::new(),
        })
    }

    async fn fetch_usage(&self, account: &Account) -> Result<UsageSnapshot, LimitLaneError> {
        let token = crate::storage::KeyringStore::get_secret("claude", &account.id)?
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok());

        if let Some(tok) = token {
            let client = reqwest::Client::new();
            let mut req = client.get(format!("{}/v1/users/me/usage", self.base_url))
                .header("anthropic-version", "2023-06-01");
            
            if tok.starts_with("sk-") {
                req = req.header("x-api-key", &tok);
            } else {
                req = req.header("Authorization", format!("Bearer {}", tok));
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().await.map_err(|e| LimitLaneError::Network {
                        url: format!("{}/v1/users/me/usage", self.base_url),
                        message: e.to_string(),
                    })?;
                    return self.parse_usage_json(&account.id, &text);
                }
                Ok(resp) => {
                    let status = resp.status();
                    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                        return Err(LimitLaneError::Authentication {
                            provider: "claude".into(),
                            account_id: account.id.clone(),
                            message: format!("Authentication failed (HTTP {})", status),
                        });
                    }
                }
                Err(e) => {
                    return Err(LimitLaneError::Network {
                        url: format!("{}/v1/users/me/usage", self.base_url),
                        message: e.to_string(),
                    });
                }
            }
        }

        let snapshot = UsageSnapshot {
            account_id: account.id.clone(),
            provider: self.provider_id(),
            plan_raw: account.plan_raw.clone(),
            observed_at: Utc::now(),
            windows: Vec::new(),
            limiting_window_id: None,
            effective_used_percent: None,
            hard_limit_reached: false,
            availability: AccountAvailability::Unknown,
            additional_usage: None,
            source: ObservationSource::Direct,
            confidence: ObservationConfidence::High,
            raw_metadata: BTreeMap::new(),
        };
        Ok(snapshot)
    }

    async fn validate_auth(&self, _account: &Account) -> Result<AccountHealth, LimitLaneError> {
        Ok(AccountHealth::Ready)
    }

    fn oauth_login_url(&self) -> Option<String> {
        Some("https://claude.ai/login".to_string())
    }
}
