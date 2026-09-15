//! Pi Agent 接入回归测试。
//!
//! 覆盖面最大的一条是 registry 级路由：make_connector 漏配 "pi" 时会静默落到
//! DetectOnlyConnector（只探测、MCP 恒空），单测连接器本身发现不了，故必须走 Registry。

use std::path::Path;

use agenthub_core::connector::{generic::GenericJsonMcpConnector, Connector};
use agenthub_core::model::McpServerDef;
use serde_json::json;

fn tmp_home() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

/// 写入形状取自本机真实 ~/.pi/agent/mcp.json（含 pi 私有的 requestTimeoutMs）
fn fixture_pi_mcp(home: &Path) -> std::path::PathBuf {
    let cfg = home.join(".pi").join("agent").join("mcp.json");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    std::fs::write(
        &cfg,
        serde_json::to_string_pretty(&json!({
            "mcpServers": {
                "MRO_API": {
                    "type": "stdio",
                    "command": "cmd",
                    "args": ["/c", "npx", "-y", "apifox-mcp-server@latest", "--project-id=8285162"],
                    "env": { "APIFOX_ACCESS_TOKEN": "afxp_local" }
                },
                "remote-docs": {
                    "type": "http",
                    "url": "https://example.com/mcp",
                    "requestTimeoutMs": 50000
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    cfg
}

fn fixture_pi_skills(home: &Path) {
    let pi_skill = home.join(".pi").join("agent").join("skills").join("image-gen");
    std::fs::create_dir_all(&pi_skill).unwrap();
    std::fs::write(
        pi_skill.join("SKILL.md"),
        "---\nname: image-gen\ndescription: 生成图片\n---\n\n正文\n",
    )
    .unwrap();

    // ~/.agents/skills 是 pi 与 ZCode 共享的目录（pi 官方 docs/skills.md 已确认）
    let shared = home.join(".agents").join("skills").join("git-commit");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(
        shared.join("SKILL.md"),
        "---\nname: git-commit\ndescription: 一键提交\n---\n",
    )
    .unwrap();
}

#[test]
fn registry_routes_pi_to_json_mcp_connector() {
    let home = tmp_home();
    fixture_pi_mcp(home.path());
    fixture_pi_skills(home.path());

    let reg = agenthub_core::registry::Registry::with_base(home.path().to_path_buf());

    let statuses = reg.statuses();
    let pi = statuses.iter().find(|s| s.id == "pi").expect("registry 应含 pi");
    assert!(pi.installed, "存在 ~/.pi/agent/mcp.json 即视为已安装");
    assert_eq!(pi.mcp_count, Some(2), "MCP 计数为 2 说明走的是 JSON 连接器而非探测兜底");
    assert_eq!(pi.skill_count, Some(2), "两个 skill 目录都应被扫到");

    let mcp = reg.all_mcp();
    let names: Vec<&str> = mcp
        .iter()
        .filter(|e| e.agent_id == "pi")
        .map(|e| e.name.as_str())
        .collect();
    assert!(names.contains(&"MRO_API") && names.contains(&"remote-docs"));
}

#[test]
fn list_mcp_reads_transports_and_pi_private_fields() {
    let home = tmp_home();
    fixture_pi_mcp(home.path());

    let conn = GenericJsonMcpConnector::new("pi", home.path().to_path_buf());
    let entries = conn.list_mcp().expect("list_mcp");

    let stdio = entries.iter().find(|e| e.name == "MRO_API").unwrap();
    assert_eq!(stdio.transport, "stdio");
    assert_eq!(stdio.command.as_deref(), Some("cmd"));
    assert_eq!(stdio.args.len(), 5);
    assert_eq!(stdio.scope, "global");

    let http = entries.iter().find(|e| e.name == "remote-docs").unwrap();
    assert_eq!(http.transport, "http");
    assert_eq!(http.url.as_deref(), Some("https://example.com/mcp"));
    // pi 私有的 requestTimeoutMs 必须原样留在 raw 里，供编辑面板回填
    assert_eq!(http.raw["requestTimeoutMs"], 50000);
}

#[test]
fn upsert_keeps_existing_servers_and_key_order() {
    let home = tmp_home();
    let cfg = fixture_pi_mcp(home.path());

    let conn = GenericJsonMcpConnector::new("pi", home.path().to_path_buf());
    let def = McpServerDef {
        command: Some("npx".into()),
        args: vec!["-y".into(), "12306-mcp".into()],
        ..Default::default()
    };
    conn.upsert_mcp("12306", &def, "global").expect("upsert");

    let after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    let top: Vec<&str> = after.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(top, vec!["mcpServers"], "顶层结构不得被改写");
    assert_eq!(after["mcpServers"]["MRO_API"]["command"], "cmd", "既有条目不得丢失");
    assert_eq!(after["mcpServers"]["remote-docs"]["url"], "https://example.com/mcp");
    assert_eq!(after["mcpServers"]["12306"]["args"][1], "12306-mcp");
}

#[test]
fn remove_mcp_drops_only_the_named_server() {
    let home = tmp_home();
    let cfg = fixture_pi_mcp(home.path());

    let conn = GenericJsonMcpConnector::new("pi", home.path().to_path_buf());
    conn.remove_mcp("remote-docs", "global").expect("remove");

    let after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    assert!(after["mcpServers"].get("remote-docs").is_none());
    assert!(after["mcpServers"].get("MRO_API").is_some());

    // 再删同一个应报 NotFound，而不是静默成功
    assert!(conn.remove_mcp("remote-docs", "global").is_err());
}

#[test]
fn skills_listed_under_pi_include_shared_agents_dir() {
    let home = tmp_home();
    fixture_pi_mcp(home.path());
    fixture_pi_skills(home.path());

    let conn = GenericJsonMcpConnector::new("pi", home.path().to_path_buf());
    let skills = conn.list_skills().expect("list_skills");
    let mut names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["git-commit", "image-gen"]);
    assert!(skills.iter().all(|s| s.agent_id == "pi" && s.has_skill_md));
}

/// 与其他 Agent 一致的红线：配置缺失时报 NotFound，绝不擅自新建 pi 的配置文件
/// （mcp.json 由 pi-mcp-adapter 归属，凭空造文件可能与其初始化逻辑打架）
#[test]
fn missing_config_is_not_created_implicitly() {
    let home = tmp_home();
    let conn = GenericJsonMcpConnector::new("pi", home.path().to_path_buf());

    assert!(conn.list_mcp().expect("未安装时读取应为空").is_empty());

    let def = McpServerDef {
        command: Some("npx".into()),
        ..Default::default()
    };
    let err = conn.upsert_mcp("x", &def, "global").unwrap_err().to_string();
    assert!(err.contains("Pi"), "错误信息应指名 Agent：{err}");
    assert!(
        !home.path().join(".pi").join("agent").join("mcp.json").exists(),
        "不得新建配置文件"
    );
}
