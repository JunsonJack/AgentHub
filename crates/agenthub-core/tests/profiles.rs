//! Profile 应用测试：skill 下发 + MCP 密钥保护（目标本地 token 不得被覆盖）。

use agenthub_core::profiles;
use agenthub_core::registry::Registry;
use agenthub_core::store::Store;
use serde_json::json;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn profile_apply_preserves_target_local_secrets() {
    let home = tmp();
    let data = tmp();

    // 中央库放一个 skill
    let stage = data.path().join("stage").join("my-skill");
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(stage.join("SKILL.md"), "---\nname: my-skill\n---\n").unwrap();
    agenthub_core::library::adopt_into(data.path(), &stage, "skills.sh", false).unwrap();

    // 源（claude）与目标（zcode）都有 yapi，token 不同
    let src_cfg = home.path().join(".claude.json");
    std::fs::write(
        &src_cfg,
        json!({
            "mcpServers": {
                "yapi": { "command": "npx", "args": ["-y"], "env": { "YAPI_TOKEN": "SRC_TOKEN", "YAPI_URL": "https://y.example" } }
            }
        })
        .to_string(),
    )
    .unwrap();
    let tgt_cfg = home.path().join(".zcode/cli/config.json");
    std::fs::create_dir_all(tgt_cfg.parent().unwrap()).unwrap();
    std::fs::write(
        &tgt_cfg,
        json!({
            "mcp": { "servers": {
                "yapi": { "command": "npx", "args": ["-y"], "env": { "YAPI_TOKEN": "TGT_TOKEN" } }
            } }
        })
        .to_string(),
    )
    .unwrap();

    // 创建 Profile：skill my-skill + mcp yapi（定义取自 claude 的条目）
    let store = Store::open(&data.path().join("t.db")).unwrap();
    let reg = Registry::with_base(home.path().to_path_buf());
    let pid = store.add_profile("e2e").unwrap();
    store.add_profile_item(pid, "skill", "my-skill", "{}").unwrap();
    let yapi_def = json!({
        "command": "npx", "args": ["-y"],
        "env": { "YAPI_TOKEN": "SRC_TOKEN", "YAPI_URL": "https://y.example" }
    });
    store
        .add_profile_item(pid, "mcp", "yapi", &yapi_def.to_string())
        .unwrap();

    let results = profiles::apply(&reg, &store, data.path(), pid, &["zcode".to_string()], false).unwrap();
    assert!(results.iter().all(|r| r.ok), "{results:?}");

    // 断言：目标本地 token 保留；非密钥同步；skill 落位
    let after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&tgt_cfg).unwrap()).unwrap();
    let env = &after["mcp"]["servers"]["yapi"]["env"];
    assert_eq!(env["YAPI_TOKEN"], "TGT_TOKEN", "目标本地密钥不得被 Profile 覆盖");
    assert_eq!(env["YAPI_URL"], "https://y.example");
    assert!(home.path().join(".zcode/skills/my-skill/SKILL.md").is_file());
}
