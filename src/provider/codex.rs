use std::collections::BTreeMap;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::evaluator::evaluate_limiting_window;
use crate::domain::types::*;
use crate::error::LimitLaneError;
use crate::provider::adapter::ProviderAdapter;

#[derive(Debug, Clone)]
pub struct CodexAdapter {
    pub base_url: String,
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self {
            base_url: "https://api.openai.com".to_string(),
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
                provider: "codex".to_string(),
                message: format!("Failed to parse JSON: {}", e),
            }
        })?;

        let obj = value.as_object().ok_or_else(|| LimitLaneError::Parsing {
            provider: "codex".to_string(),
            message: "Root JSON is not an object".to_string(),
        })?;

        let plan_raw = obj
            .get("plan")
            .or_else(|| obj.get("plan_type"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let now = Utc::now();
        let mut windows = Vec::new();

        // 1. Shared agentic pool top-level object
        if let Some(pool_obj) = obj.get("shared_agentic_pool").and_then(|v| v.as_object()) {
            let used_percent = pool_obj.get("used_percent").and_then(|v| v.as_f64());
            let remaining_percent = used_percent.map(|u| (100.0 - u).max(0.0));

            let reset_at = pool_obj
                .get("reset_at")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            let duration_seconds = pool_obj.get("duration_seconds").and_then(|v| v.as_u64());

            let mut provider_metadata = BTreeMap::new();
            for (k, v) in pool_obj {
                if k != "used_percent" && k != "reset_at" && k != "duration_seconds" {
                    provider_metadata.insert(k.clone(), v.to_string());
                }
            }

            windows.push(UsageWindow {
                id: "shared_agentic_pool".to_string(),
                raw_provider_id: Some("shared_agentic_pool".to_string()),
                label: "Shared Agentic Pool".to_string(),
                scope: UsageScope::SharedAgenticPool,
                window_type: WindowType::ProviderDefined,
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

        // 2. Windows or limits array
        if let Some(windows_arr) = obj
            .get("windows")
            .or_else(|| obj.get("limits"))
            .and_then(|v| v.as_array())
        {
            for (idx, win_val) in windows_arr.iter().enumerate() {
                if let Some(win_obj) = win_val.as_object() {
                    let raw_id = win_obj
                        .get("id")
                        .or_else(|| win_obj.get("name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let id = raw_id.clone().unwrap_or_else(|| format!("window_{}", idx));

                    let used_percent = win_obj.get("used_percent").and_then(|v| v.as_f64());
                    let remaining_percent = win_obj
                        .get("remaining_percent")
                        .and_then(|v| v.as_f64())
                        .or_else(|| used_percent.map(|u| (100.0 - u).max(0.0)));

                    let consumed_units = win_obj.get("consumed_units").and_then(|v| v.as_f64());
                    let limit_units = win_obj.get("limit_units").and_then(|v| v.as_f64());

                    let reset_at = win_obj
                        .get("reset_at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc));

                    let duration_seconds = win_obj
                        .get("duration_seconds")
                        .or_else(|| win_obj.get("duration"))
                        .and_then(|v| v.as_u64());

                    let scope = parse_codex_scope(win_obj);
                    let window_type = parse_codex_window_type(win_obj);
                    let enforcement = parse_codex_enforcement(win_obj);
                    let unit = parse_codex_unit(win_obj);

                    let label = win_obj
                        .get("label")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format_codex_label(&id, &scope));

                    let mut provider_metadata = BTreeMap::new();
                    for (k, v) in win_obj {
                        if !is_standard_window_key(k) {
                            provider_metadata.insert(k.clone(), v.to_string());
                        }
                    }

                    windows.push(UsageWindow {
                        id,
                        raw_provider_id: raw_id,
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
                        observed_at: now,
                        source: ObservationSource::Direct,
                        confidence: ObservationConfidence::High,
                        provider_metadata,
                    });
                }
            }
        }

        // 3. Additional usage / credit balance
        let additional_usage = parse_codex_additional_usage(obj);

        // 4. Raw metadata preservation
        let mut raw_metadata = BTreeMap::new();
        for (k, v) in obj {
            if !is_standard_root_key(k) {
                raw_metadata.insert(k.clone(), v.to_string());
            }
        }

        let mut snapshot = UsageSnapshot {
            account_id: account_id.to_string(),
            provider: "codex".to_string(),
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

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn normalize_codex_plan(plan_str: &str) -> NormalizedPlan {
    match plan_str.to_lowercase().as_str() {
        "free" | "openai_free" => NormalizedPlan::OpenAiFree,
        "go" | "openai_go" => NormalizedPlan::OpenAiGo,
        "plus" | "openai_plus" => NormalizedPlan::OpenAiPlus,
        "pro" | "openai_pro" => NormalizedPlan::OpenAiPro,
        "business" | "openai_business" => NormalizedPlan::OpenAiBusiness,
        "enterprise" | "openai_enterprise" => NormalizedPlan::OpenAiEnterprise,
        "edu" | "openai_edu" => NormalizedPlan::OpenAiEdu,
        "api" => NormalizedPlan::Api,
        _ => NormalizedPlan::Unknown,
    }
}

fn parse_codex_scope(win_obj: &serde_json::Map<String, serde_json::Value>) -> UsageScope {
    if let Some(scope_str) = win_obj.get("scope").and_then(|v| v.as_str()) {
        match scope_str.to_lowercase().as_str() {
            "shared_agentic_pool" | "shared" | "shared_agentic" => UsageScope::SharedAgenticPool,
            "workspace" => UsageScope::Workspace,
            "all_account_usage" | "all" => UsageScope::AllAccountUsage,
            "all_models" => UsageScope::AllModels,
            "credits" => UsageScope::Credits,
            "spend_limit" | "spend" => UsageScope::SpendLimit,
            other => {
                if let Some(surface) = win_obj.get("product_surface").and_then(|v| v.as_str()) {
                    UsageScope::ProductSurface(surface.to_string())
                } else if other.starts_with("product_surface:") {
                    UsageScope::ProductSurface(other.trim_start_matches("product_surface:").to_string())
                } else {
                    UsageScope::Unknown(other.to_string())
                }
            }
        }
    } else if let Some(surface) = win_obj.get("product_surface").and_then(|v| v.as_str()) {
        UsageScope::ProductSurface(surface.to_string())
    } else if let Some(id_str) = win_obj.get("id").or_else(|| win_obj.get("name")).and_then(|v| v.as_str()) {
        if id_str.contains("shared_agentic_pool") || id_str.contains("shared") {
            UsageScope::SharedAgenticPool
        } else if id_str.contains("workspace") {
            UsageScope::Workspace
        } else if id_str.contains("product_surface") {
            let surf = win_obj.get("surface").and_then(|v| v.as_str()).unwrap_or(id_str);
            UsageScope::ProductSurface(surf.to_string())
        } else {
            UsageScope::AllAccountUsage
        }
    } else {
        UsageScope::AllAccountUsage
    }
}

fn parse_codex_window_type(win_obj: &serde_json::Map<String, serde_json::Value>) -> WindowType {
    if let Some(wt_str) = win_obj.get("window_type").or_else(|| win_obj.get("type")).and_then(|v| v.as_str()) {
        match wt_str.to_lowercase().as_str() {
            "rolling" => WindowType::Rolling,
            "fixed" => WindowType::Fixed,
            "credit_balance" | "credit" => WindowType::CreditBalance,
            "spend" => WindowType::Spend,
            "demand_adjusted" => WindowType::DemandAdjusted,
            "provider_defined" | "provider" => WindowType::ProviderDefined,
            _ => WindowType::Unknown,
        }
    } else {
        WindowType::ProviderDefined
    }
}

fn parse_codex_enforcement(win_obj: &serde_json::Map<String, serde_json::Value>) -> EnforcementType {
    if let Some(enf_str) = win_obj.get("enforcement").and_then(|v| v.as_str()) {
        match enf_str.to_lowercase().as_str() {
            "hard" => EnforcementType::Hard,
            "soft" => EnforcementType::Soft,
            "paid_overflow" | "paid" => EnforcementType::PaidOverflow,
            "informational" | "info" => EnforcementType::Informational,
            _ => EnforcementType::Unknown,
        }
    } else {
        EnforcementType::Hard
    }
}

fn parse_codex_unit(win_obj: &serde_json::Map<String, serde_json::Value>) -> Option<UsageUnit> {
    win_obj.get("unit").and_then(|v| v.as_str()).map(|u| {
        match u.to_lowercase().as_str() {
            "percent" => UsageUnit::Percent,
            "credits" => UsageUnit::Credits,
            "tokens" => UsageUnit::Tokens,
            "messages" => UsageUnit::Messages,
            "tasks" => UsageUnit::Tasks,
            other => UsageUnit::Unknown(other.to_string()),
        }
    })
}

fn parse_codex_additional_usage(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Option<AdditionalUsage> {
    let extra_obj = obj
        .get("extra_credits")
        .or_else(|| obj.get("additional_usage"))
        .and_then(|v| v.as_object());

    let top_credit_balance = obj.get("credit_balance").and_then(|v| v.as_f64());
    let top_spent_credits = obj.get("spent_credits").and_then(|v| v.as_f64());

    if extra_obj.is_none() && top_credit_balance.is_none() && top_spent_credits.is_none() {
        return None;
    }

    let enabled = extra_obj
        .and_then(|o| o.get("enabled").and_then(|v| v.as_bool()))
        .unwrap_or_else(|| top_credit_balance.map_or(false, |b| b > 0.0));

    let exhausted = extra_obj
        .and_then(|o| o.get("exhausted").and_then(|v| v.as_bool()))
        .or_else(|| top_credit_balance.map(|b| b <= 0.0));

    let balance_val = extra_obj
        .and_then(|o| o.get("balance").and_then(|v| v.as_f64()))
        .or(top_credit_balance);

    let spent_val = extra_obj
        .and_then(|o| o.get("spent").and_then(|v| v.as_f64()))
        .or(top_spent_credits);

    let limit_val = extra_obj
        .and_then(|o| o.get("spending_limit").and_then(|v| v.as_f64()));

    let used_percent = extra_obj
        .and_then(|o| o.get("used_percent").and_then(|v| v.as_f64()));

    let shared_scope = extra_obj
        .and_then(|o| o.get("shared_scope").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let currency = extra_obj
        .and_then(|o| o.get("currency").and_then(|v| v.as_str()))
        .unwrap_or("USD")
        .to_string();

    Some(AdditionalUsage {
        enabled,
        exhausted,
        balance: balance_val.map(|amt| Money {
            amount: amt,
            currency: currency.clone(),
        }),
        spent: spent_val.map(|amt| Money {
            amount: amt,
            currency: currency.clone(),
        }),
        spending_limit: limit_val.map(|amt| Money {
            amount: amt,
            currency,
        }),
        used_percent,
        shared_scope,
    })
}

fn is_standard_window_key(k: &str) -> bool {
    matches!(
        k,
        "id" | "name"
            | "label"
            | "scope"
            | "product_surface"
            | "window_type"
            | "type"
            | "enforcement"
            | "duration_seconds"
            | "duration"
            | "used_percent"
            | "remaining_percent"
            | "consumed_units"
            | "limit_units"
            | "unit"
            | "reset_at"
    )
}

fn is_standard_root_key(k: &str) -> bool {
    matches!(
        k,
        "account_id"
            | "plan"
            | "plan_type"
            | "shared_agentic_pool"
            | "windows"
            | "limits"
            | "extra_credits"
            | "additional_usage"
            | "credit_balance"
            | "spent_credits"
    )
}

fn format_codex_label(id: &str, scope: &UsageScope) -> String {
    match scope {
        UsageScope::SharedAgenticPool => "Shared Agentic Pool".to_string(),
        UsageScope::Workspace => "Workspace Usage".to_string(),
        UsageScope::ProductSurface(surf) => format!("Surface: {}", surf),
        _ => {
            let label = id.replace('_', " ");
            let mut c = label.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    }
}

#[async_trait]
impl ProviderAdapter for CodexAdapter {
    fn provider_id(&self) -> ProviderId {
        "codex".to_string()
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
            .map(normalize_codex_plan);

        Ok(AccountMetadata {
            plan_raw: account.plan_raw.clone(),
            plan_normalized,
            metadata: BTreeMap::new(),
        })
    }

    async fn fetch_usage(&self, account: &Account) -> Result<UsageSnapshot, LimitLaneError> {
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
}
