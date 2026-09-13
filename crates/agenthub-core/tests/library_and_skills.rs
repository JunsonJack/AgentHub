//! Skill 扫描、中央库收编、快照回滚的行为测试（全部走临时数据目录，不碰真实用户数据）。

use agenthub_core::connector::{claude_code::ClaudeCodeConnector, Connector};
use agenthub_core::library;
use agenthub_core::snapshot;

fn tmp_home() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn make_skill(home: &std::path::Path, agent_dir: &str, name: &str, desc: &str) -> std::path::PathBuf {
    let dir = home.join(agent_dir).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: \"{desc}\"\n---\n\n# 正文\n"),
    )
    .unwrap();
    std::fs::write(dir.join("helper.txt"), "x").unwrap();
    dir
}

#[test]
fn list_skills_scans_registry_dirs_with_frontmatter() {
    let home = tmp_home();
    make_skill(home.path(), ".zcode/skills", "alpha", "第一个技能");
    make_skill(home.path(), ".zcode/skills", "beta", "第二个技能");
    std::fs::write(home.path().join(".zcode/skills/not-a-skill.txt"), "x").unwrap();

    let conn = agenthub_core::connector::zcode::ZcodeConnector::new(home.path().to_path_buf());
    let skills = conn.list_skills().unwrap();
    let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "beta"], "只认目录，且按名称排序");
    assert_eq!(skills[0].description.as_deref(), Some("第一个技能"));
    assert!(skills[0].has_skill_md);
}

#[test]
fn claude_code_lists_project_level_skills() {
    let home = tmp_home();
    make_skill(home.path(), ".claude/skills", "user-skill", "用户级");
    let proj = home.path().join("E:/demo");
    let dir = proj.join(".claude/skills/proj-skill");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), "---\nname: proj-skill\ndescription: 项目级\n---\n").unwrap();
    std::fs::write(
        home.path().join(".claude.json"),
        serde_json::json!({ "projects": { "E:/demo": {} } }).to_string(),
    )
    .unwrap();

    let conn = ClaudeCodeConnector::new(home.path().to_path_buf());
    let skills = conn.list_skills().unwrap();
    let scopes: Vec<&str> = skills.iter().map(|s| s.scope.as_str()).collect();
    assert!(scopes.contains(&"user"));
    assert!(scopes.contains(&"project:E:/demo"));
}

#[test]
fn adopt_dry_run_then_real_then_conflict() {
    let home = tmp_home();
    let data = tmp_home();
    let src = make_skill(home.path(), ".claude/skills", "my-skill", "测试收编");

    // 1) dry-run：只报告，不落盘
    let plan = library::adopt_into(data.path(), &src, "claude-code", true).unwrap();
    assert!(!plan.conflict);
    assert!(plan.files.contains(&"SKILL.md".to_string()));
    assert!(
        !library::skills_root(data.path()).join("my-skill").exists(),
        "dry-run 不得写盘"
    );

    // 2) 真实收编
    let report = library::adopt_into(data.path(), &src, "claude-code", false).unwrap();
    assert!(!report.conflict);
    let target = library::skills_root(data.path()).join("my-skill");
    assert!(target.join("SKILL.md").is_file());
    assert!(target.join("helper.txt").is_file());
    assert!(target.join("manifest.json").is_file());

    let items = library::list_library_in(data.path());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source_agent.as_deref(), Some("claude-code"));
    assert_eq!(items[0].description.as_deref(), Some("测试收编"));
    let md = library::read_skill_md_in(data.path(), "my-skill").unwrap();
    assert!(md.contains("# 正文"), "库内条目 SKILL.md 可读");

    // 3) 重复收编 = 冲突，拒绝覆盖
    let again = library::adopt_into(data.path(), &src, "claude-code", false).unwrap();
    assert!(again.conflict);
}

#[test]
fn snapshot_manifest_list_and_rollback() {
    let home = tmp_home();
    let data = tmp_home();
    let cfg = home.path().join(".claude.json");
    std::fs::write(&cfg, r#"{"version":1}"#).unwrap();

    // 写入动作触发快照（模拟 connector 行为）
    let backup = snapshot::snapshot_in(data.path(), &cfg).unwrap();
    assert!(backup.is_file());

    // 当前列表可见，manifest 记录原路径
    let metas = snapshot::list_in(data.path());
    assert_eq!(metas.len(), 1);
    assert_eq!(metas[0].original_path, cfg.display().to_string());

    // 修改原文件后回滚
    std::fs::write(&cfg, r#"{"version":2,"broken":true}"#).unwrap();
    let restored = snapshot::rollback_in(data.path(), &metas[0].id, &metas[0].file_name).unwrap();
    assert_eq!(restored, cfg);
    let content = std::fs::read_to_string(&cfg).unwrap();
    assert!(content.contains("\"version\":1"), "应恢复为备份内容");

    // 回滚本身也留了快照（可撤销），所以现在有 2 条记录
    assert_eq!(snapshot::list_in(data.path()).len(), 2);
}

#[test]
fn validate_def_rejects_incomplete() {
    use agenthub_core::connector::validate_def;
    use agenthub_core::model::McpServerDef;

    assert!(validate_def(&McpServerDef::default()).is_err(), "stdio 无 command 应拒绝");
    assert!(validate_def(&McpServerDef { command: Some("npx".into()), ..Default::default() }).is_ok());
    assert!(validate_def(&McpServerDef { url: Some("https://x".into()), ..Default::default() }).is_ok());
}
