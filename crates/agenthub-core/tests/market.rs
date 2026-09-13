//! 市场模块离线测试：解析、URL 处理、安装落盘、deploy_skill（全部临时目录，不碰网络）。

use agenthub_core::library;
use agenthub_core::market;
use agenthub_core::model::MarketSkill;
use agenthub_core::registry::Registry;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn parse_git_url_tree_links() {
    let spec = market::parse_git_url(
        "https://github.com/openclaw/openclaw/tree/main/extensions/browser/skills/browser-automation",
    )
    .unwrap();
    assert_eq!(spec.repo_url, "https://github.com/openclaw/openclaw");
    assert_eq!(spec.branch.as_deref(), Some("main"));
    assert_eq!(
        spec.subpath.as_deref(),
        Some("extensions/browser/skills/browser-automation")
    );

    let spec2 = market::parse_git_url("https://github.com/owner/repo#skills/my-skill").unwrap();
    assert_eq!(spec2.repo_url, "https://github.com/owner/repo");
    assert_eq!(spec2.branch, None);
    assert_eq!(spec2.subpath.as_deref(), Some("skills/my-skill"));

    let spec3 = market::parse_git_url("https://github.com/owner/repo.git").unwrap();
    assert_eq!(spec3.subpath, None);

    assert!(market::parse_git_url("").is_err());
}

#[test]
fn skillsmp_json_shape_parses() {
    // 真实响应结构（2026-09 实测采样）
    let raw = r#"{
      "success": true,
      "data": { "skills": [ {
        "id": "openclaw-openclaw-extensions-browser-skills-browser-automation-skill-md",
        "name": "browser-automation",
        "author": "openclaw",
        "description": "Use when controlling web pages with the OpenClaw browser tool.",
        "contentLanguage": "en",
        "githubUrl": "https://github.com/openclaw/openclaw/tree/main/extensions/browser/skills/browser-automation",
        "skillUrl": "https://skillsmp.com/creators/openclaw/openclaw",
        "stars": 389030,
        "updatedAt": 1787945027
      } ] },
      "meta": { "requestId": "x", "responseTimeMs": 4 }
    }"#;
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(v["data"]["skills"][0]["name"], "browser-automation");

    let err_raw = r#"{ "success": false, "error": { "code": "INVALID_API_KEY", "message": "bad key" } }"#;
    let ev: serde_json::Value = serde_json::from_str(err_raw).unwrap();
    assert_eq!(ev["error"]["code"], "INVALID_API_KEY");
}

#[test]
fn skills_sh_download_snapshot_writes_to_library_with_conflict() {
    let data = tmp();
    // install_skills_sh 的核心路径：stage 落盘 + adopt_into 入库
    // 暂存目录名 = skill 名（真实流程中由 slug 命名）
    let stage = data.path().join("stage-tmp").join("git-commit");
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(
        stage.join("SKILL.md"),
        "---\nname: git-commit\ndescription: 'Execute git commit'\n---\n\nbody\n",
    )
    .unwrap();
    std::fs::create_dir_all(stage.join("scripts")).unwrap();
    std::fs::write(stage.join("scripts/run.sh"), "echo hi").unwrap();

    let report = library::adopt_into(data.path(), &stage, "skills.sh", false).unwrap();
    assert!(!report.conflict);
    assert_eq!(report.skill_name, "git-commit");
    let lib_item = library::skills_root(data.path()).join("git-commit");
    assert!(lib_item.join("SKILL.md").is_file());
    assert!(lib_item.join("scripts/run.sh").is_file());

    let items = library::list_library_in(data.path());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "git-commit");
    assert_eq!(items[0].source_agent.as_deref(), Some("skills.sh"));
    assert_eq!(items[0].file_count, 2);

    // 重复安装 = 冲突，拒绝覆盖
    let again = library::adopt_into(data.path(), &stage, "skills.sh", false).unwrap();
    assert!(again.conflict);
}

#[test]
fn deploy_skill_copies_and_refuses_overwrite() {
    let home = tmp();
    let data = tmp();
    std::fs::create_dir_all(home.path().join(".zcode/skills")).unwrap();

    let stage = data.path().join("stage-tmp").join("demo");
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(stage.join("SKILL.md"), "---\nname: demo\n---\n").unwrap();
    library::adopt_into(data.path(), &stage, "skills.sh", false).unwrap();

    let reg = Registry::with_base(home.path().to_path_buf());
    // 首次安装成功
    let out = reg.deploy_skill_in(data.path(), "demo", &["zcode".to_string()], false);
    assert!(out[0].ok, "首次安装应成功: {:?}", out[0].error);
    let target = home.path().join(".zcode/skills/demo");
    assert!(target.join("SKILL.md").is_file());

    // 重复安装不覆盖 → 拒绝
    let again = reg.deploy_skill_in(data.path(), "demo", &["zcode".to_string()], false);
    assert!(!again[0].ok);
    assert!(again[0].error.as_deref().unwrap_or_default().contains("拒绝覆盖"));

    // 显式 overwrite → 允许
    let over = reg.deploy_skill_in(data.path(), "demo", &["zcode".to_string()], true);
    assert!(over[0].ok, "覆盖安装应成功: {:?}", over[0].error);

    // 库里不存在的条目
    let missing = reg.deploy_skill_in(data.path(), "nope", &["zcode".to_string()], false);
    assert!(!missing[0].ok);
}

#[test]
fn store_settings_roundtrip() {
    let data = tmp();
    let store = agenthub_core::store::Store::open(&data.path().join("t.db")).unwrap();
    assert_eq!(store.get_setting("skillsmp_key").unwrap(), None);
    store.set_setting("skillsmp_key", "sk_live_abc123def").unwrap();
    assert_eq!(
        store.get_setting("skillsmp_key").unwrap().as_deref(),
        Some("sk_live_abc123def")
    );
    store.set_setting("skillsmp_key", "sk_live_second").unwrap();
    assert_eq!(store.get_setting("skillsmp_key").unwrap().as_deref(), Some("sk_live_second"));
    store.delete_setting("skillsmp_key").unwrap();
    assert_eq!(store.get_setting("skillsmp_key").unwrap(), None);
}

#[test]
fn market_skill_model_serializes_camel_case() {
    let s = MarketSkill {
        id: "a/b/c".into(),
        name: "c".into(),
        market: "skills.sh".into(),
        source: Some("a/b".into()),
        author: None,
        description: None,
        installs: Some(1234),
        stars: None,
        github_url: None,
    };
    let j = serde_json::to_value(&s).unwrap();
    assert_eq!(j["installs"], 1234);
    assert_eq!(j["market"], "skills.sh");
}
