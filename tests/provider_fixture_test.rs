use serde_json::Value;

#[test]
fn test_claude_pro_fixture_loads() {
    let content = include_str!("fixtures/claude_pro.json");
    let v: Value = serde_json::from_str(content).expect("claude_pro.json should be valid JSON");
    
    assert_eq!(v["account_id"], "dev@example.com");
    assert_eq!(v["plan"], "max_5x");
    assert_eq!(v["extra_credits"]["enabled"], true);
    assert_eq!(v["extra_credits"]["exhausted"], false);

    let limits = v["limits"].as_array().expect("limits should be array");
    assert_eq!(limits.len(), 3);
    assert_eq!(limits[0]["used_percent"], 43.0);
    assert_eq!(limits[1]["used_percent"], 61.0);
    assert_eq!(limits[2]["used_percent"], 82.0);
}

#[test]
fn test_codex_plus_fixture_loads() {
    let content = include_str!("fixtures/codex_plus.json");
    let v: Value = serde_json::from_str(content).expect("codex_plus.json should be valid JSON");

    assert_eq!(v["account_id"], "personal@example.com");
    assert_eq!(v["plan"], "plus");
    assert_eq!(v["shared_agentic_pool"]["used_percent"], 74.0);
    assert_eq!(v["credit_balance"], 1250);
    assert_eq!(v["extra_credits"]["enabled"], true);
    assert_eq!(v["extra_credits"]["balance"], 1250.0);
}
