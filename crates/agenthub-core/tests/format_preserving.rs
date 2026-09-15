//! 格式保留写入的回归测试（规划红线：绝不 parse 后整体重写破坏注释与排版）。

use agenthub_core::connector::{claude_code::ClaudeCodeConnector, codex::CodexConnector, Connector};
use agenthub_core::model::McpServerDef;
use serde_json::json;

fn tmp_home() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn claude_code_upsert_keeps_key_order_and_adds_server() {
    let home = tmp_home();
    let cfg = home.path().join(".claude.json");
    let original = json!({
        "numStartups": 42,
        "mcpServers": {
            "aaa": { "command": "npx", "args": ["-y", "aaa-mcp"] }
        },
        "projects": {},
        "userID": "u-1"
    });
    std::fs::write(
        &cfg,
        serde_json::to_string_pretty(&original).unwrap(),
    )
    .unwrap();

    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let def = McpServerDef {
        command: Some("uvx".into()),
        args: vec!["mcp-server-fetch".into()],
        ..Default::default()
    };
    conn.upsert_mcp("fetch", &def, "global").expect("upsert");

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    let keys: Vec<&str> = after.as_object().unwrap().keys().map(String::as_str).collect();
    // 顶层键序保持：upsert 只改 mcpServers 内部，不动顶层结构
    assert_eq!(keys, vec!["numStartups", "mcpServers", "projects", "userID"]);
    assert!(after["mcpServers"]["fetch"]["command"] == "uvx");
    assert!(after["mcpServers"]["aaa"]["command"] == "npx", "旧条目不得丢失");
}

#[test]
fn codex_upsert_preserves_comments_and_layout() {
    let home = tmp_home();
    let cfg = home.path().join(".codex").join("config.toml");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    let original = r#"# 顶部注释：模型供应商
[model_providers.custom]
name = "custom"

# MCP：YApi 文档
[mcp_servers.yapi]
command = "npx"
args = ["-y", "yapi-mcp"]
"#;
    std::fs::write(&cfg, original).unwrap();

    let conn = CodexConnector::new(home.path().to_path_buf());
    let def = McpServerDef {
        command: Some("uvx".into()),
        args: vec!["mcp-server-fetch".into()],
        ..Default::default()
    };
    conn.upsert_mcp("fetch", &def, "global").expect("upsert");

    let after = std::fs::read_to_string(&cfg).unwrap();
    assert!(after.contains("# 顶部注释：模型供应商"), "顶层注释必须保留");
    assert!(after.contains("# MCP：YApi 文档"), "节前注释必须保留");
    assert!(after.contains("[mcp_servers.yapi]"), "原有节必须保留");
    assert!(after.contains("[mcp_servers.fetch]"), "新节必须写入");
    // 列出全部条目确认无损
    let entries = conn.list_mcp().expect("list");
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"yapi") && names.contains(&"fetch"));
}

#[test]
fn codex_remove_only_touches_target() {
    let home = tmp_home();
    let cfg = home.path().join(".codex").join("config.toml");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    let original = "[mcp_servers.a]\ncommand = \"a\"\n\n[mcp_servers.b]\ncommand = \"b\"\n";
    std::fs::write(&cfg, original).unwrap();

    let conn = CodexConnector::new(home.path().to_path_buf());
    conn.remove_mcp("a", "global").expect("remove");
    let after = std::fs::read_to_string(&cfg).unwrap();
    assert!(!after.contains("[mcp_servers.a]"));
    assert!(after.contains("[mcp_servers.b]"), "未删除的条目必须原样保留");
}

#[test]
fn snapshot_created_before_write() {
    let home = tmp_home();
    let cfg = home.path().join(".claude.json");
    std::fs::write(&cfg, "{\"mcpServers\":{}}").unwrap();
    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let report = conn
        .upsert_mcp("x", &McpServerDef { command: Some("c".into()), ..Default::default() }, "global")
        .expect("upsert");
    let backup = report.backup_path.expect("必须有备份路径");
    assert!(std::path::Path::new(&backup).exists(), "备份文件必须真实存在");
}

#[test]
fn claude_code_reads_project_scope() {
    let home = tmp_home();
    let cfg = home.path().join(".claude.json");
    std::fs::write(
        &cfg,
        json!({
            "mcpServers": { "g1": { "command": "g" } },
            "projects": {
                "E:/demo": { "mcpServers": { "p1": { "command": "p" } } }
            }
        })
        .to_string(),
    )
    .unwrap();
    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let entries = conn.list_mcp().expect("list");
    let scopes: Vec<(&str, &str)> =
        entries.iter().map(|e| (e.name.as_str(), e.scope.as_str())).collect();
    assert!(scopes.contains(&("g1", "global")));
    assert!(scopes.contains(&("p1", "project:E:/demo")));
}

#[test]
fn claude_code_upsert_project_scope() {
    use agenthub_core::registry::Registry;
    let home = tmp_home();
    let cfg = home.path().join(".claude.json");
    std::fs::write(
        &cfg,
        json!({
            "mcpServers": {},
            "projects": {
                "E:/demo": { "mcpServers": {} }
            }
        })
        .to_string(),
    )
    .unwrap();

    let reg = Registry::with_base(home.path().to_path_buf());
    let def = McpServerDef {
        command: Some("npx".into()),
        args: vec!["-y".into(), "proj-mcp".into()],
        ..Default::default()
    };
    let results = reg.deploy_mcp(
        &["claude-code".to_string()],
        "proj-server",
        &def,
        Some("project:E:/demo"),
    );
    assert!(results[0].ok, "deploy failed: {:?}", results[0].error);

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    assert!(after["projects"]["E:/demo"]["mcpServers"]["proj-server"]["command"] == "npx");
    // 不得误写到全局
    assert!(after["mcpServers"].as_object().unwrap().is_empty());

    // 删除项目级
    reg.remove_mcp_for("claude-code", "proj-server", "project:E:/demo")
        .expect("remove project mcp");
    let after2: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    assert!(after2["projects"]["E:/demo"]["mcpServers"]
        .as_object()
        .unwrap()
        .is_empty());
}

#[test]
fn claude_code_upsert_project_creates_project_node() {
    use agenthub_core::connector::claude_code::ClaudeCodeConnector;
    let home = tmp_home();
    let cfg = home.path().join(".claude.json");
    std::fs::write(&cfg, r#"{"mcpServers":{}}"#).unwrap();
    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let def = McpServerDef {
        command: Some("c".into()),
        ..Default::default()
    };
    conn.upsert_mcp("new", &def, "project:E:/fresh")
        .expect("upsert creates project node");
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    assert!(after["projects"]["E:/fresh"]["mcpServers"]["new"]["command"] == "c");
}

#[test]
fn cursor_reads_jsonc_and_writes() {
    use agenthub_core::connector::cursor::CursorConnector;
    let home = tmp_home();
    let cfg = home.path().join(".cursor").join("mcp.json");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    // 官方风格：带注释的 JSONC
    let original = r#"{
  // 全局 MCP
  "mcpServers": {
    "existing": {
      "command": "npx",
      "args": ["-y", "existing-mcp"]
    }
  }
}
"#;
    std::fs::write(&cfg, original).unwrap();

    let conn = CursorConnector::new(home.path().to_path_buf());
    let entries = conn.list_mcp().expect("must parse JSONC");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "existing");

    let def = McpServerDef {
        command: Some("uvx".into()),
        args: vec!["fetch".into()],
        ..Default::default()
    };
    conn.upsert_mcp("fetch", &def, "global").expect("write");
    let after = std::fs::read_to_string(&cfg).unwrap();
    // 注释在整体重写时无法保留（有快照），但结构必须完整
    let v: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert!(v["mcpServers"]["fetch"]["command"] == "uvx");
    assert!(v["mcpServers"]["existing"]["command"] == "npx");
}

#[test]
fn cursor_missing_file_does_not_create() {
    use agenthub_core::connector::cursor::CursorConnector;
    use agenthub_core::error::CoreError;
    let home = tmp_home();
    let conn = CursorConnector::new(home.path().to_path_buf());
    let def = McpServerDef {
        command: Some("c".into()),
        ..Default::default()
    };
    let err = conn.upsert_mcp("x", &def, "global").unwrap_err();
    assert!(matches!(err, CoreError::NotFound(_)));
}
