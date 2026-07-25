use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::cli::args::*;
use crate::doctor::{format_text_report, run_doctor_checks};
use crate::domain::types::*;
use crate::error::LimitLaneError;
use crate::provider::adapter::ProviderAdapter;
use crate::provider::claude::ClaudeAdapter;
use crate::provider::codex::CodexAdapter;
use crate::service::refresh::{RefreshConfig, RefreshService};
use crate::storage::{DatabaseRepository, KeyringStore};

#[derive(Serialize)]
pub struct AccountStatusPair<'a> {
    pub account: &'a Account,
    pub latest_snapshot: Option<&'a UsageSnapshot>,
}

#[derive(Serialize)]
pub struct StatusOutput<'a> {
    pub schema_version: &'static str,
    pub timestamp: DateTime<Utc>,
    pub accounts: Vec<AccountStatusPair<'a>>,
}

#[derive(Serialize)]
pub struct AccountsOutput<'a> {
    pub schema_version: &'static str,
    pub accounts: &'a [Account],
}

#[derive(Serialize)]
pub struct UsageOutput<'a> {
    pub schema_version: &'static str,
    pub snapshots: &'a [UsageSnapshot],
}

#[derive(Serialize)]
pub struct DoctorOutput<'a> {
    pub schema_version: &'static str,
    pub report: &'a crate::doctor::DiagnosticReport,
}

pub fn format_status_json(
    accounts: &[Account],
    snapshots: &[UsageSnapshot],
) -> Result<String, LimitLaneError> {
    let mut account_pairs = Vec::new();
    for acc in accounts {
        let snap = snapshots.iter().find(|s| s.account_id == acc.id);
        account_pairs.push(AccountStatusPair {
            account: acc,
            latest_snapshot: snap,
        });
    }

    let output = StatusOutput {
        schema_version: "1",
        timestamp: Utc::now(),
        accounts: account_pairs,
    };

    serde_json::to_string_pretty(&output)
        .map_err(|e| LimitLaneError::Parsing {
            provider: "cli".into(),
            message: e.to_string(),
        })
}

pub fn format_accounts_json(accounts: &[Account]) -> Result<String, LimitLaneError> {
    let output = AccountsOutput {
        schema_version: "1",
        accounts,
    };

    serde_json::to_string_pretty(&output)
        .map_err(|e| LimitLaneError::Parsing {
            provider: "cli".into(),
            message: e.to_string(),
        })
}

pub fn format_usage_json(snapshots: &[UsageSnapshot]) -> Result<String, LimitLaneError> {
    let output = UsageOutput {
        schema_version: "1",
        snapshots,
    };

    serde_json::to_string_pretty(&output)
        .map_err(|e| LimitLaneError::Parsing {
            provider: "cli".into(),
            message: e.to_string(),
        })
}

pub async fn run_cli_command(
    cli: Cli,
    db: &DatabaseRepository,
    _keyring: &KeyringStore,
) -> Result<(), LimitLaneError> {
    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            println!("LimitLane v{}", env!("CARGO_PKG_VERSION"));
            println!("Run 'limitlane --help' for usage instructions.");
            return Ok(());
        }
    };

    match command {
        Commands::Status(args) => {
            let accounts = db.list_accounts()?;
            let mut snapshots = Vec::new();
            for acc in &accounts {
                if let Some(snap) = db.get_latest_snapshot(&acc.id)? {
                    snapshots.push(snap);
                }
            }

            if args.json {
                println!("{}", format_status_json(&accounts, &snapshots)?);
            } else {
                println!("================================================================================");
                println!("                         LimitLane Account Usage Status                         ");
                println!("================================================================================");
                if accounts.is_empty() {
                    println!("No accounts configured. Use 'limitlane accounts add' to register an account.");
                } else {
                    println!("{:<16} {:<10} {:<24} {:<12} {:<10} {:<12}", "ID", "PROVIDER", "IDENTITY", "PLAN", "USED %", "HEALTH");
                    println!("--------------------------------------------------------------------------------");
                    for acc in &accounts {
                        let snap = snapshots.iter().find(|s| s.account_id == acc.id);
                        let used_str = snap
                            .and_then(|s| s.effective_used_percent)
                            .map(|u| format!("{:.1}%", u))
                            .unwrap_or_else(|| "N/A".into());
                        let plan_str = acc.plan_raw.as_deref().unwrap_or("Unknown");
                        println!(
                            "{:<16} {:<10} {:<24} {:<12} {:<10} {:?}",
                            acc.id, acc.provider, acc.identity, plan_str, used_str, acc.health
                        );
                    }
                }
                println!("--------------------------------------------------------------------------------");
            }
        }
        Commands::Accounts(args) => {
            let action = args.action.unwrap_or(AccountAction::List { json: args.json });
            match action {
                AccountAction::List { json } => {
                    let show_json = json || args.json;
                    let accounts = db.list_accounts()?;
                    if show_json {
                        println!("{}", format_accounts_json(&accounts)?);
                    } else {
                        println!("================================================================================");
                        println!("                             Configured Accounts                                ");
                        println!("================================================================================");
                        if accounts.is_empty() {
                            println!("No accounts configured.");
                        } else {
                            println!("{:<16} {:<10} {:<24} {:<12} {:<12}", "ID", "PROVIDER", "IDENTITY", "ALIAS", "AUTH");
                            println!("--------------------------------------------------------------------------------");
                            for acc in &accounts {
                                let alias = acc.alias.as_deref().unwrap_or("-");
                                println!(
                                    "{:<16} {:<10} {:<24} {:<12} {:?}",
                                    acc.id, acc.provider, acc.identity, alias, acc.auth_method
                                );
                            }
                        }
                        println!("--------------------------------------------------------------------------------");
                    }
                }
                AccountAction::Add {
                    provider,
                    identity,
                    p,
                    i,
                    alias,
                    auth,
                    api_key,
                } => {
                    let prov = provider.or(p).ok_or_else(|| {
                        LimitLaneError::Configuration("Missing provider argument. Example: limitlane accounts add codex yoshinoya@gmail.com".into())
                    })?;
                    let ident = identity.or(i).ok_or_else(|| {
                        LimitLaneError::Configuration("Missing identity argument. Example: limitlane accounts add codex yoshinoya@gmail.com".into())
                    })?;

                    let sanitized_id = format!("{}_{}", prov, ident.replace('@', "_at_").replace('.', "_"));
                    let auth_method = match auth.to_lowercase().as_str() {
                        "oauth" => AuthMethod::OAuth,
                        "session_token" | "session" => AuthMethod::SessionToken,
                        _ => AuthMethod::ApiKey,
                    };

                    if let Some(ref key) = api_key {
                        KeyringStore::store_secret(&prov, &sanitized_id, key)?;
                    }

                    let account = Account {
                        id: sanitized_id.clone(),
                        provider: prov.clone(),
                        identity: ident.clone(),
                        alias,
                        auth_method: auth_method.clone(),
                        plan_raw: None,
                        plan_normalized: None,
                        active: Some(true),
                        health: AccountHealth::Ready,
                        credential_reference: api_key.as_ref().map(|_| format!("keyring://{}/{}", prov, sanitized_id)),
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                        last_refresh_at: None,
                    };

                    db.save_account(&account)?;
                    println!("Successfully added account '{}' for provider '{}'.", account.id, prov);

                    if auth_method == AuthMethod::OAuth {
                        let oauth_url = match prov.to_lowercase().as_str() {
                            "claude" => "https://claude.ai/login",
                            _ => "https://auth.openai.com/authorize",
                        };
                        println!("\n================================================================================");
                        println!("                           OAuth Authorization Link                             ");
                        println!("================================================================================");
                        println!("Please open the following link in your browser to complete authorization:\n");
                        println!("  {}", oauth_url);
                        println!("\nLimitLane will automatically detect your active OAuth session from official client state.");
                        println!("--------------------------------------------------------------------------------\n");
                    }
                }
                AccountAction::Remove { id } => {
                    db.delete_account(&id)?;
                    println!("Successfully removed account '{}'.", id);
                }
                AccountAction::Reauth { id } => {
                    if let Some(mut acc) = db.get_account(&id)? {
                        acc.health = AccountHealth::ReauthenticationRequired;
                        acc.updated_at = Utc::now();
                        db.save_account(&acc)?;
                        let oauth_url = match acc.provider.to_lowercase().as_str() {
                            "claude" => "https://claude.ai/login",
                            _ => "https://auth.openai.com/authorize",
                        };
                        println!("================================================================================");
                        println!("                      OAuth Re-authorization Link                               ");
                        println!("================================================================================");
                        println!("Account '{}' ({}) marked for re-authentication.", acc.id, acc.provider);
                        println!("\nPlease open this link in your browser to re-authorize:\n");
                        println!("  {}", oauth_url);
                        println!("\n--------------------------------------------------------------------------------");
                    } else {
                        return Err(LimitLaneError::Configuration(format!("Account '{}' not found", id)));
                    }
                }
            }
        }
        Commands::Usage(args) => {
            let accounts = db.list_accounts()?;
            let filtered: Vec<_> = accounts
                .into_iter()
                .filter(|a| {
                    if let Some(ref acc_id) = args.account {
                        if &a.id != acc_id {
                            return false;
                        }
                    }
                    if let Some(ref prov) = args.provider {
                        if &a.provider != prov {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            let mut snapshots = Vec::new();
            for acc in &filtered {
                if let Some(snap) = db.get_latest_snapshot(&acc.id)? {
                    snapshots.push(snap);
                }
            }

            if args.json {
                println!("{}", format_usage_json(&snapshots)?);
            } else {
                println!("================================================================================");
                println!("                           Detailed Usage Snapshots                             ");
                println!("================================================================================");
                if snapshots.is_empty() {
                    println!("No usage snapshots found for selected filter.");
                } else {
                    for snap in &snapshots {
                        println!(
                            "\nAccount: {} ({}) | Plan: {} | Availability: {:?}",
                            snap.account_id,
                            snap.provider,
                            snap.plan_raw.as_deref().unwrap_or("Unknown"),
                            snap.availability
                        );
                        println!("{:<24} {:<12} {:<12} {:<24}", "WINDOW LABEL", "USED %", "ENFORCEMENT", "RESET TIME");
                        println!("--------------------------------------------------------------------------------");
                        for w in &snap.windows {
                            let used_str = w.used_percent.map(|u| format!("{:.1}%", u)).unwrap_or_else(|| "N/A".into());
                            let reset_str = w.reset_at.map(|r| r.format("%Y-%m-%d %H:%M:%S UTC").to_string()).unwrap_or_else(|| "N/A".into());
                            println!("{:<24} {:<12} {:<12?} {:<24}", w.label, used_str, w.enforcement, reset_str);
                        }
                    }
                }
                println!("\n--------------------------------------------------------------------------------");
            }
        }
        Commands::Refresh(args) => {
            let accounts = db.list_accounts()?;
            let to_refresh: Vec<_> = accounts
                .into_iter()
                .filter(|a| {
                    if let Some(ref acc_id) = args.account {
                        if &a.id != acc_id {
                            return false;
                        }
                    }
                    if let Some(ref prov) = args.provider {
                        if &a.provider != prov {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            if to_refresh.is_empty() {
                println!("No matching accounts found to refresh.");
                return Ok(());
            }

            let mut providers: HashMap<String, Arc<dyn ProviderAdapter>> = HashMap::new();
            providers.insert("claude".to_string(), Arc::new(ClaudeAdapter::new()));
            providers.insert("codex".to_string(), Arc::new(CodexAdapter::new()));

            let service = RefreshService::new(Arc::new(db.clone()), providers, RefreshConfig::default());
            println!("Triggering usage refresh for {} account(s)...", to_refresh.len());
            for acc in &to_refresh {
                match service.refresh_account(&acc.id).await {
                    Ok(snap) => {
                        println!("  [OK] Account '{}' refreshed. Effective used: {:?}", acc.id, snap.effective_used_percent);
                    }
                    Err(e) => {
                        println!("  [FAIL] Account '{}' refresh failed: {}", acc.id, e);
                    }
                }
            }
        }
        Commands::Doctor(args) => {
            let report = run_doctor_checks(Some(db)).await;
            if args.json {
                let output = DoctorOutput {
                    schema_version: "1",
                    report: &report,
                };
                println!("{}", serde_json::to_string_pretty(&output).map_err(|e| LimitLaneError::Parsing { provider: "cli".into(), message: e.to_string() })?);
            } else {
                println!("{}", format_text_report(&report));
            }
        }
        Commands::Config(args) => {
            println!("LimitLane Configuration");
            println!("Action: {}", args.action);
            if let Some(proj_dirs) = directories::ProjectDirs::from("com", "LimitLane", "LimitLane") {
                println!("Config directory: {}", proj_dirs.config_dir().display());
                println!("Data directory:   {}", proj_dirs.data_local_dir().display());
            }
        }
        Commands::Version => {
            println!("LimitLane v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
