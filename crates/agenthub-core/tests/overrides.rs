//! 路径覆写：把 Agent 的配置路径指向自定义位置。

use agenthub_core::registry::Registry;
use serde_json::json;

#[test]
fn override_redirects_connector_to_custom_config() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();

    // 自定义位置放一份 Claude Code 配置
    let custom_cfg = data.path().join("custom").join("my-claude.json");
    std::fs::create_dir_all(custom_cfg.parent().unwrap()).unwrap();
    std::fs::write(
        &custom_cfg,
        json!({ "mcpServers": { "custom-srv": { "command": "echo" } } }).to_string(),
    )
    .unwrap();

    let overrides = json!({
        "claude-code": {
            "mcpConfigPaths": [custom_cfg.display().to_string()]
        }
    });
    let reg = Registry::with_overrides(home.path().to_path_buf(), &overrides);

    let entry = reg
        .all_mcp()
        .into_iter()
        .find(|e| e.agent_id == "claude-code" && e.name == "custom-srv")
        .expect("覆写后应能从自定义路径读到条目");
    assert_eq!(entry.transport, "stdio");

    // 默认构造（无覆写）则读不到
    let reg_default = Registry::with_base(home.path().to_path_buf());
    assert!(
        !reg_default
            .all_mcp()
            .iter()
            .any(|e| e.agent_id == "claude-code" && e.name == "custom-srv")
    );
}

#[test]
fn override_skill_dirs_changes_skill_scan() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let custom_skills = data.path().join("my-skills");
    std::fs::create_dir_all(custom_skills.join("mine")).unwrap();
    std::fs::write(custom_skills.join("mine").join("SKILL.md"), "---\nname: mine\n---\n").unwrap();

    let overrides = json!({ "zcode": { "skillDirs": [custom_skills.display().to_string()] } });
    let reg = Registry::with_overrides(home.path().to_path_buf(), &overrides);
    let skills = reg.all_skills();
    assert!(skills.iter().any(|s| s.agent_id == "zcode" && s.name == "mine"));
}

#[test]
fn invalid_override_json_rejected() {
    // set_path_overrides 的校验逻辑在 tauri 层；此处验证空对象无副作用
    let home = tempfile::tempdir().unwrap();
    let reg = Registry::with_overrides(home.path().to_path_buf(), &json!({}));
    assert!(!reg.statuses().is_empty(), "空覆写不影响默认行为");
}
