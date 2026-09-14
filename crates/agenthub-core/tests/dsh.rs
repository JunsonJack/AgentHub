use std::path::Path;

use agenthub_core::connector::{dsh::DshConnector, Connector};
use agenthub_core::model::{AgentDescriptor, AgentKind, McpServerDef, OsPaths};
use serde_json::json;

fn desc() -> AgentDescriptor {
    AgentDescriptor {
        id: "dsh".into(), name: "DeepSeek Harness".into(), kind: AgentKind::Cli,
        mcp_config_paths: OsPaths { windows: vec!["~/.dsh/dsh-mcp.json".into()], macos: vec![], linux: vec![] },
        skill_dirs: OsPaths::default(), mcp_format: "dsh-array".into(), reload: "restart".into(),
    }
}

fn fixture(home: &Path) -> std::path::PathBuf {
    let path = home.join(".dsh/dsh-mcp.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let root = json!({
        "version": 1,
        "servers": [
            {"name":"MRO", "transport":"stdio", "enabled":true, "toolCallTimeoutMs":15000,
             "reconnect":{}, "command":"cmd", "args":["/c","npx","-y","mro"],
             "cwd":"E:/project", "env":{"TOKEN":"secret"}},
            {"name":"remote", "transport":"http", "enabled":false, "url":"https://example.com/mcp",
             "customDshField":{"keep":true}}
        ],
        "unknownTopLevel": "must-keep"
    });
    std::fs::write(&path, serde_json::to_string_pretty(&root).unwrap()).unwrap();
    path
}

#[test]
fn dsh_reads_array_and_preserves_metadata() {
    let home = tempfile::tempdir().unwrap();
    fixture(home.path());
    let conn = DshConnector::with_descriptor(desc(), home.path().to_path_buf());
    let entries = conn.list_mcp().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "MRO");
    assert_eq!(entries[0].transport, "stdio");
    assert_eq!(entries[0].command.as_deref(), Some("cmd"));
    assert_eq!(entries[0].raw["enabled"], true);
    assert_eq!(entries[0].raw["cwd"], "E:/project");
    assert_eq!(entries[1].transport, "http");
}

#[test]
fn dsh_upsert_merges_standard_fields_without_dropping_private_fields() {
    let home = tempfile::tempdir().unwrap();
    let path = fixture(home.path());
    let conn = DshConnector::with_descriptor(desc(), home.path().to_path_buf());
    let def = McpServerDef { command: Some("new-cmd".into()), args: vec!["--new".into()], ..Default::default() };
    conn.upsert_mcp("MRO", &def).unwrap();
    let root: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mro = &root["servers"][0];
    assert_eq!(mro["command"], "new-cmd");
    assert_eq!(mro["enabled"], true);
    assert_eq!(mro["cwd"], "E:/project");
    assert_eq!(mro["env"]["TOKEN"], "secret");
    assert_eq!(root["unknownTopLevel"], "must-keep");
}

#[test]
fn dsh_upsert_adds_and_remove_only_named_server() {
    let home = tempfile::tempdir().unwrap();
    let path = fixture(home.path());
    let conn = DshConnector::with_descriptor(desc(), home.path().to_path_buf());
    let def = McpServerDef { url: Some("https://new.example/mcp".into()), ..Default::default() };
    conn.upsert_mcp("new", &def).unwrap();
    conn.remove_mcp("remote", "global").unwrap();
    let root: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let names: Vec<_> = root["servers"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["MRO", "new"]);
    assert_eq!(root["servers"][1]["url"], "https://new.example/mcp");
}

#[test]
fn dsh_missing_config_does_not_create_file() {
    let home = tempfile::tempdir().unwrap();
    let conn = DshConnector::with_descriptor(desc(), home.path().to_path_buf());
    assert!(conn.list_mcp().unwrap().is_empty());
    let err = conn.upsert_mcp("x", &McpServerDef::default()).unwrap_err().to_string();
    assert!(err.contains("DeepSeek Harness"));
}
