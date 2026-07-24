use limitlane::doctor::checker::{format_text_report, run_doctor_checks, CheckStatus};
use limitlane::storage::DatabaseRepository;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_run_doctor_checks_without_db() {
    let report = run_doctor_checks(None).await;

    assert_eq!(report.os.name, "Operating System");
    assert_eq!(report.architecture.name, "Architecture");
    assert_eq!(report.config_dir.name, "Configuration Directory");
    assert_eq!(report.database.name, "SQLite Database Access");
    assert_eq!(report.keyring.name, "OS Keyring Storage");
    assert_eq!(report.codex_cli.name, "Codex CLI");
    assert_eq!(report.claude_code.name, "Claude Code CLI");
    assert_eq!(report.network.name, "Network Connectivity");
    assert_eq!(report.clock_skew.name, "Clock Synchronization");
    assert_eq!(report.redacted_logging.name, "Redacted Logging");
}

#[tokio::test]
async fn test_run_doctor_checks_with_db() {
    let tmp = NamedTempFile::new().unwrap();
    let db = DatabaseRepository::open(tmp.path()).unwrap();
    db.migrate().unwrap();

    let report = run_doctor_checks(Some(&db)).await;

    assert_eq!(report.database.status, CheckStatus::Ok);
    assert!(report.database.message.contains("accessible") || report.database.message.contains("OK"));
}

#[tokio::test]
async fn test_diagnostic_report_json_serialization() {
    let report = run_doctor_checks(None).await;
    let json_str = serde_json::to_string_pretty(&report).expect("Failed to serialize DiagnosticReport");
    let json_val: serde_json::Value = serde_json::from_str(&json_str).expect("Failed to parse JSON output");

    // Verify required fields in JSON serialization
    assert!(json_val.get("os").is_some());
    assert!(json_val.get("architecture").is_some());
    assert!(json_val.get("terminal").is_some());
    assert!(json_val.get("config_dir").is_some());
    assert!(json_val.get("database").is_some());
    assert!(json_val.get("keyring").is_some());
    assert!(json_val.get("codex_cli").is_some());
    assert!(json_val.get("claude_code").is_some());
    assert!(json_val.get("network").is_some());
    assert!(json_val.get("clock_skew").is_some());
    assert!(json_val.get("redacted_logging").is_some());
    assert!(json_val.get("overall_health").is_some());
}

#[tokio::test]
async fn test_format_text_report() {
    let report = run_doctor_checks(None).await;
    let text = format_text_report(&report);

    assert!(text.contains("LimitLane Diagnostic Report"));
    assert!(text.contains("Operating System"));
    assert!(text.contains("Overall Health"));
}
