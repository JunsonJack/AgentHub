//! 版本更新测试：用本地 git 仓库当上游，全程离线确定性。

use agenthub_core::library;
use agenthub_core::registry::Registry;
use agenthub_core::updater;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn git(cwd: &std::path::Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git 可用");
    assert!(out.status.success(), "git {:?} 失败: {}", args, String::from_utf8_lossy(&out.stderr));
}

/// 建本地上游仓库：git init + 提交（user.name/email 用 -c 内联配置）
fn make_upstream(dir: &std::path::Path, skill_md: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), skill_md).unwrap();
    git(dir, &["init", "-q"]);
    git(dir, &["-c", "user.name=t", "-c", "user.email=t@t.t", "add", "-A"]);
    git(dir, &["-c", "user.name=t", "-c", "user.email=t@t.t", "commit", "-q", "-m", "init"]);
}

#[test]
fn check_and_apply_update_from_git_upstream() {
    let home = tmp();
    let data = tmp();
    let upstream = tmp();

    // v1 上游
    make_upstream(&upstream.path().join("repo"), "---\nname: upd\n---\nv1\n");
    let repo_url = upstream.path().join("repo").to_string_lossy().replace('\\', "/");

    // 安装 v1 入库
    let report = agenthub_core::market::install_git(&repo_url, data.path(), false).unwrap();
    assert_eq!(report.skill_name, "repo");

    // 初次检查：应一致
    let c1 = updater::check_update(data.path(), "repo").unwrap();
    assert!(c1.checkable && c1.identical, "{c1:?}");

    // 上游升级 v2 + 新增文件
    std::fs::write(upstream.path().join("repo").join("SKILL.md"), "---\nname: upd\n---\nv2\n").unwrap();
    std::fs::write(upstream.path().join("repo").join("extra.md"), "new file").unwrap();
    git(&upstream.path().join("repo"), &["-c", "user.name=t", "-c", "user.email=t@t.t", "add", "-A"]);
    git(&upstream.path().join("repo"), &["-c", "user.name=t", "-c", "user.email=t@t.t", "commit", "-q", "-m", "v2"]);

    // 检查：发现更新（check 只读）
    let c2 = updater::check_update(data.path(), "repo").unwrap();
    assert!(c2.has_updates, "{c2:?}");
    assert!(c2.incoming.contains(&"extra.md".to_string()));
    assert!(c2.changed.iter().any(|f| f == "SKILL.md"), "SKILL.md 属于变更文件: {c2:?}");
    assert_eq!(
        std::fs::read_to_string(library::skills_root(data.path()).join("repo").join("SKILL.md")).unwrap(),
        "---\nname: upd\n---\nv1\n",
        "check 阶段不得改动中央库"
    );

    // 应用更新
    let r = updater::apply_update(data.path(), "repo", &Registry::with_base(home.path().to_path_buf()), false, false).unwrap();
    assert!(r.updated_library);
    let lib_md = std::fs::read_to_string(library::skills_root(data.path()).join("repo").join("SKILL.md")).unwrap();
    assert!(lib_md.contains("v2"));
    assert!(library::skills_root(data.path()).join("repo").join("extra.md").is_file());
    // manifest 记录更新时间
    let items = library::list_library_in(data.path());
    assert!(items.iter().find(|i| i.name == "repo").unwrap().updated_at.is_some());

    // 再查：一致
    let c3 = updater::check_update(data.path(), "repo").unwrap();
    assert!(c3.identical);
}

#[test]
fn apply_update_syncs_to_deployed_agent_with_holdback() {
    let home = tmp();
    let data = tmp();
    let upstream = tmp();

    make_upstream(&upstream.path().join("repo"), "v1\n");
    let repo_url = upstream.path().join("repo").to_string_lossy().replace('\\', "/");
    agenthub_core::market::install_git(&repo_url, data.path(), false).unwrap();

    // Agent（zcode）已部署 v1 + 有一个本地多出的文件
    let tgt = home.path().join(".zcode/skills/repo");
    std::fs::create_dir_all(&tgt).unwrap();
    std::fs::write(tgt.join("SKILL.md"), "v1\n").unwrap();
    std::fs::write(tgt.join("local-only.txt"), "keep me").unwrap();

    // 上游 v2
    std::fs::write(upstream.path().join("repo").join("SKILL.md"), "v2\n").unwrap();
    git(&upstream.path().join("repo"), &["-c", "user.name=t", "-c", "user.email=t@t.t", "add", "-A"]);
    git(&upstream.path().join("repo"), &["-c", "user.name=t", "-c", "user.email=t@t.t", "commit", "-q", "-m", "v2"]);

    let reg = Registry::with_base(home.path().to_path_buf());
    let r = updater::apply_update(data.path(), "repo", &reg, true, false).unwrap();
    assert!(r.updated_library);
    assert_eq!(r.synced.len(), 1, "应同步到 zcode");
    let s = &r.synced[0];
    assert_eq!(s.copied, 1, "SKILL.md 应更新");
    assert_eq!(
        s.held_back_deletions,
        vec!["local-only.txt".to_string()],
        "目标多出的文件必须扣留"
    );
    assert!(tgt.join("local-only.txt").is_file(), "未确认删除前保留");
    assert!(std::fs::read_to_string(tgt.join("SKILL.md")).unwrap().contains("v2"));
}

#[test]
fn non_checkable_source_reports_error() {
    let data = tmp();
    // 手工造一个无上游来源的条目（收编自本地目录）
    let stage = data.path().join("stage").join("local-skill");
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(stage.join("SKILL.md"), "x").unwrap();
    library::adopt_into(data.path(), &stage, "claude-code", false).unwrap();

    let c = updater::check_update(data.path(), "local-skill").unwrap();
    assert!(!c.checkable);
    assert!(c.error.is_some());
}
