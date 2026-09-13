//! 健康度检查测试（临时目录，不碰真实配置）。

use agenthub_core::connector::claude_code::ClaudeCodeConnector;
use agenthub_core::health::{check_connector, find_on_path};

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn codes(v: &[agenthub_core::model::HealthIssue]) -> Vec<String> {
    v.iter().map(|i| i.code.clone()).collect()
}

#[test]
fn broken_json_config_surfaces_parse_error() {
    let home = tmp();
    std::fs::write(home.path().join(".claude.json"), "{ not valid json ,, ").unwrap();
    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let issues = check_connector(&conn);
    assert!(
        codes(&issues).contains(&"CONFIG_PARSE_ERROR".to_string()),
        "损坏配置必须报 CONFIG_PARSE_ERROR，实际: {issues:?}"
    );
}

#[test]
fn invalid_entries_are_flagged() {
    let home = tmp();
    let cfg = home.path().join(".claude.json");
    std::fs::write(
        &cfg,
        serde_json::json!({
            "mcpServers": {
                "no-target": { "args": ["x"] },
                "empty-cmd": { "command": "  " }
            }
        })
        .to_string(),
    )
    .unwrap();
    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let issues = check_connector(&conn);
    let invalid = issues
        .iter()
        .filter(|i| i.code == "MCP_ENTRY_INVALID")
        .count();
    assert!(invalid >= 2, "两个非法条目都应被发现: {issues:?}");
}

#[test]
fn missing_command_on_path_is_warning() {
    let home = tmp();
    let cfg = home.path().join(".claude.json");
    std::fs::write(
        &cfg,
        serde_json::json!({
            "mcpServers": {
                "ghost": { "command": "definitely-not-a-real-binary-xyz-9527" }
            }
        })
        .to_string(),
    )
    .unwrap();
    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let issues = check_connector(&conn);
    assert!(codes(&issues).contains(&"MCP_CMD_NOT_FOUND".to_string()));
    // 是 warning 不是 error（npx 拉取型命令缺席不致命）
    let warn = issues
        .iter()
        .find(|i| i.code == "MCP_CMD_NOT_FOUND")
        .unwrap();
    assert_eq!(warn.severity, "warning");
}

#[test]
fn path_lookup_behaves() {
    assert!(find_on_path("definitely-not-a-real-binary-xyz-9527") == false);
    // cmd/where 一定在 Windows PATH 上；跨平台时 sh 也行
    let known = if cfg!(windows) { "cmd" } else { "sh" };
    assert!(find_on_path(known), "{known} 应该能在 PATH 上找到");
}

#[test]
fn skill_without_skill_md_flagged() {
    let home = tmp();
    let dir = home.path().join(".claude/skills/broken-skill");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(home.path().join(".claude.json"), "{\"mcpServers\":{}}").unwrap();
    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let issues = check_connector(&conn);
    assert!(codes(&issues).contains(&"SKILL_NO_SKILLMD".to_string()));
}
