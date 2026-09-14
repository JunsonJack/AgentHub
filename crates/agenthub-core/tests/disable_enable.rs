//! MCP 禁用/启用测试：移出配置 + 记录 + 一键还原（含 TOML 注释保留往返）。

use agenthub_core::connector::{claude_code::ClaudeCodeConnector, Connector};
use agenthub_core::registry::Registry;
use agenthub_core::store::Store;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn disable_removes_from_config_and_enable_restores() {
    let home = tmp();
    let data = tmp();
    let cfg = home.path().join(".claude.json");
    std::fs::write(
        &cfg,
        serde_json::json!({
            "userID": "u1",
            "mcpServers": { "yapi": { "command": "npx", "args": ["-y", "yapi-mcp"] } }
        })
        .to_string(),
    )
    .unwrap();
    let store = Store::open(&data.path().join("t.db")).unwrap();
    let reg = Registry::with_base(home.path().to_path_buf());

    // 禁用：配置中消失 + 有记录
    reg.disable_mcp(&store, "claude-code", "yapi", "global").unwrap();
    let content = std::fs::read_to_string(&cfg).unwrap();
    assert!(!content.contains("yapi"), "禁用后配置不得再含该条目");
    assert!(content.contains("userID"), "其他键不得被破坏");
    let records = store.list_disabled_mcp().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "yapi");

    // 启用：写回原样
    reg.enable_mcp(&store, records[0].id).unwrap();
    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let entries = conn.list_mcp().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "yapi");
    assert_eq!(entries[0].args, vec!["-y", "yapi-mcp"]);
    assert!(store.list_disabled_mcp().unwrap().is_empty());
}

#[test]
fn codex_disable_enable_preserves_comments() {
    let home = tmp();
    let data = tmp();
    let cfg = home.path().join(".codex/config.toml");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    let original = "# 顶部注释\n[model_providers.custom]\nname = \"c\"\n\n# YApi 文档服务\n[mcp_servers.yapi]\ncommand = \"npx\"\nargs = [\"-y\", \"yapi-mcp\"]\n";
    std::fs::write(&cfg, original).unwrap();

    let store = Store::open(&data.path().join("t.db")).unwrap();
    let reg = Registry::with_base(home.path().to_path_buf());

    reg.disable_mcp(&store, "codex", "yapi", "global").unwrap();
    let after_disable = std::fs::read_to_string(&cfg).unwrap();
    assert!(!after_disable.contains("[mcp_servers.yapi]"));
    assert!(after_disable.contains("# 顶部注释"), "注释必须保留");
    assert!(after_disable.contains("[model_providers.custom]"), "其他节保留");

    reg.enable_mcp(&store, store.list_disabled_mcp().unwrap()[0].id).unwrap();
    let after_enable = std::fs::read_to_string(&cfg).unwrap();
    // 注：被禁用条目自己的注释（# YApi 文档服务）随条目一起移除——这是"移除该条目"的一部分；
    // 无关内容（顶部注释、其他节）必须原样保留。条目本身与字段值完整还原。
    assert!(after_enable.contains("[mcp_servers.yapi]"));
    assert!(after_enable.contains("command = \"npx\""));
    assert!(after_enable.contains("# 顶部注释"));
    assert!(after_enable.contains("[model_providers.custom]"));
}

#[test]
fn disable_unknown_entry_errors_and_records_nothing() {
    let home = tmp();
    let data = tmp();
    std::fs::write(home.path().join(".claude.json"), "{\"mcpServers\":{}}").unwrap();
    let store = Store::open(&data.path().join("t.db")).unwrap();
    let reg = Registry::with_base(home.path().to_path_buf());
    assert!(reg.disable_mcp(&store, "claude-code", "ghost", "global").is_err());
    assert!(store.list_disabled_mcp().unwrap().is_empty());
}

#[test]
fn enable_unknown_record_errors() {
    let data = tmp();
    let store = Store::open(&data.path().join("t.db")).unwrap();
    let reg = Registry::with_base(std::env::temp_dir());
    assert!(reg.enable_mcp(&store, 99999).is_err());
}
