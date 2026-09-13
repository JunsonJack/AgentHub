//! 同步引擎测试：diff、计划、传播（删除扣留）、MCP env 密钥保护。全部临时目录。

use agenthub_core::registry::Registry;
use agenthub_core::store::Store;
use agenthub_core::sync;
use serde_json::json;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

/// 两台"机器"：home_src / home_tgt 各自是一个临时 HOME；data 是共享的中央数据根
struct Env {
    _home_src: tempfile::TempDir,
    _home_tgt: tempfile::TempDir,
    _data: tempfile::TempDir,
    reg: Registry,
}

fn setup() -> Env {
    let home_src = tmp();
    let home_tgt = tmp();
    let data = tmp();
    // Registry 的 base 是单个目录——用 with_base(src) 测源侧，tgt 用独立 Registry？
    // 桌面应用里源和目标在同一台机器（同一个 HOME），测试里也让它们同 HOME：
    // 源 = claude-code，目标 = zcode，各自 skill 目录都在同一个 home 下。
    let reg = Registry::with_base(home_src.path().to_path_buf());
    Env {
        _home_src: home_src,
        _home_tgt: home_tgt,
        _data: data,
        reg,
    }
}

fn write_skill(home: &std::path::Path, agent_dir: &str, name: &str, files: &[(&str, &str)]) {
    for (rel, content) in files {
        let p = home.join(agent_dir).join(name).join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }
}

#[test]
fn plan_and_apply_sync_with_deletion_holdback() {
    let env = setup();
    let home = env._home_src.path().to_path_buf();

    // 源：SKILL.md(v2) + helper.txt；目标：SKILL.md(v1) + stale.txt（源里没有）
    write_skill(
        &home,
        ".claude/skills",
        "demo",
        &[("SKILL.md", "v2"), ("helper.txt", "hello")],
    );
    write_skill(
        &home,
        ".zcode/skills",
        "demo",
        &[("SKILL.md", "v1"), ("stale.txt", "old")],
    );

    // 计划（dry-run，只读）
    let plan = sync::plan_skill_sync(&env.reg, "claude-code", "demo", "user", "zcode").unwrap();
    assert!(plan.copy.contains(&"SKILL.md".to_string()));
    assert!(plan.copy.contains(&"helper.txt".to_string()));
    assert_eq!(plan.deletions, vec!["stale.txt".to_string()]);
    assert!(!plan.target_absent);
    assert!(!plan.identical);

    // 计划阶段不得改动目标
    assert_eq!(
        std::fs::read_to_string(home.join(".zcode/skills/demo/SKILL.md")).unwrap(),
        "v1"
    );

    // 执行（不确认删除）：复制 + 扣留删除 + 目录快照
    let report = sync::apply_skill_sync(&env.reg, "claude-code", "demo", "user", &plan, false).unwrap();
    assert_eq!(report.copied, 2);
    assert_eq!(report.deleted, 0);
    assert_eq!(report.held_back_deletions, vec!["stale.txt".to_string()]);
    assert!(report.backup_dir.is_some(), "同步前必须有目录快照");
    assert!(
        std::path::Path::new(report.backup_dir.as_ref().unwrap())
            .join("stale.txt")
            .is_file(),
        "快照里应保留原内容"
    );
    assert_eq!(
        std::fs::read_to_string(home.join(".zcode/skills/demo/SKILL.md")).unwrap(),
        "v2",
        "SKILL.md 应更新为源版本"
    );
    assert!(
        home.join(".zcode/skills/demo/stale.txt").is_file(),
        "未确认删除前，多余文件必须保留"
    );

    // 确认删除后再同步
    let report2 = sync::apply_skill_sync(&env.reg, "claude-code", "demo", "user", &plan, true).unwrap();
    assert_eq!(report2.deleted, 1);
    assert!(!home.join(".zcode/skills/demo/stale.txt").exists());
}

#[test]
fn sync_to_agent_without_skill_installs_fresh() {
    let env = setup();
    let home = env._home_src.path().to_path_buf();
    write_skill(&home, ".claude/skills", "fresh", &[("SKILL.md", "x")]);

    let plan = sync::plan_skill_sync(&env.reg, "claude-code", "fresh", "user", "zcode").unwrap();
    assert!(plan.target_absent);
    assert_eq!(plan.copy, vec!["SKILL.md".to_string()]);

    sync::apply_skill_sync(&env.reg, "claude-code", "fresh", "user", &plan, false).unwrap();
    assert!(home.join(".zcode/skills/fresh/SKILL.md").is_file());
}

#[test]
fn identical_sync_is_noop() {
    let env = setup();
    let home = env._home_src.path().to_path_buf();
    write_skill(&home, ".claude/skills", "same", &[("SKILL.md", "same")]);
    write_skill(&home, ".zcode/skills", "same", &[("SKILL.md", "same")]);

    let plan = sync::plan_skill_sync(&env.reg, "claude-code", "same", "user", "zcode").unwrap();
    assert!(plan.identical);
    assert!(plan.copy.is_empty() && plan.deletions.is_empty());
}

#[test]
fn mcp_propagate_keeps_target_env_values() {
    let home = tmp();
    let data = tmp();
    // 源（claude-code）：YAPI 带 token=SRC_TOKEN
    let src_cfg = home.path().join(".claude.json");
    std::fs::write(
        &src_cfg,
        json!({
            "mcpServers": {
                "yapi": { "command": "npx", "args": ["-y", "yapi-mcp"], "env": { "YAPI_TOKEN": "SRC_TOKEN", "YAPI_URL": "https://y.example" } }
            }
        })
        .to_string(),
    )
    .unwrap();
    // 目标（zcode）：已有 yapi，本地 token=TGT_TOKEN
    let tgt_cfg = home.path().join(".zcode/cli/config.json");
    std::fs::create_dir_all(tgt_cfg.parent().unwrap()).unwrap();
    std::fs::write(
        &tgt_cfg,
        json!({
            "mcp": { "servers": {
                "yapi": { "command": "npx", "args": ["-y", "yapi-mcp"], "env": { "YAPI_TOKEN": "TGT_TOKEN" } }
            } }
        })
        .to_string(),
    )
    .unwrap();

    let store = Store::open(&data.path().join("t.db")).unwrap();
    let _ = store;
    let reg = Registry::with_base(home.path().to_path_buf());
    let results = sync::propagate_mcp(&reg, "claude-code", "yapi", &["zcode".to_string()]);
    assert!(results[0].ok, "传播应成功: {:?}", results[0].error);

    // 验证目标配置：键对齐，但本地值保留
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&tgt_cfg).unwrap()).unwrap();
    let env = &after["mcp"]["servers"]["yapi"]["env"];
    assert_eq!(env["YAPI_TOKEN"], "TGT_TOKEN", "目标本地密钥不得被覆盖");
    assert_eq!(env["YAPI_URL"], "https://y.example", "非密钥值照常同步");
    let src_after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&src_cfg).unwrap()).unwrap();
    assert_eq!(
        src_after["mcpServers"]["yapi"]["env"]["YAPI_TOKEN"],
        "SRC_TOKEN",
        "源侧不得被改动"
    );
}

#[test]
fn mcp_propagate_to_new_target_uses_placeholder() {
    let home = tmp();
    let src_cfg = home.path().join(".claude.json");
    std::fs::write(
        &src_cfg,
        json!({
            "mcpServers": {
                "yapi": { "command": "npx", "args": ["-y"], "env": { "YAPI_TOKEN": "SECRET" } }
            }
        })
        .to_string(),
    )
    .unwrap();
    // 目标已有配置文件（空 servers）
    let tgt_cfg = home.path().join(".zcode/cli/config.json");
    std::fs::create_dir_all(tgt_cfg.parent().unwrap()).unwrap();
    std::fs::write(&tgt_cfg, json!({"mcp": {"servers": {}}}).to_string()).unwrap();

    let reg = Registry::with_base(home.path().to_path_buf());
    let results = sync::propagate_mcp(&reg, "claude-code", "yapi", &["zcode".to_string()]);
    assert!(results[0].ok, "{:?}", results[0].error);

    let tgt: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&tgt_cfg).unwrap()).unwrap();
    let env = &tgt["mcp"]["servers"]["yapi"]["env"];
    assert_eq!(
        env["YAPI_TOKEN"],
        "<请在目标 Agent 中填写>",
        "新目标的密钥必须是占位符，绝不携带明文"
    );
}
