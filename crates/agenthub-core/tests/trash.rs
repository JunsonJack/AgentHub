//! 回收站测试：删除→恢复→冲突拒绝（全部临时目录，不碰真实用户数据）。

use agenthub_core::registry::Registry;
use agenthub_core::trash;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn make_home_with_skill() -> (tempfile::TempDir, std::path::PathBuf) {
    let home = tmp();
    let skill_dir = home.path().join(".claude/skills/doomed-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "---\nname: doomed\n---\n").unwrap();
    std::fs::write(skill_dir.join("extra.txt"), "data").unwrap();
    (home, skill_dir)
}

#[test]
fn delete_then_restore_roundtrip() {
    let (home, skill_dir) = make_home_with_skill();
    let data = tmp();
    let trash_root = data.path().join("trash");
    std::fs::write(home.path().join(".claude.json"), "{\"mcpServers\":{}}").unwrap();

    let reg = Registry::with_base(home.path().to_path_buf());
    let item = reg
        .remove_skill_in(&trash_root, "claude-code", "doomed-skill", "user")
        .expect("remove 应成功");

    assert!(!skill_dir.exists(), "原目录应已移走");
    assert!(trash_root.join(&item.id).join("data").join("SKILL.md").is_file());
    assert_eq!(item.name, "doomed-skill");

    let items = trash::list_trash_in(&trash_root);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, item.id);
    assert_eq!(items[0].agent_id, "claude-code");

    let restored = trash::restore_trash_in(&trash_root, &item.id).unwrap();
    assert_eq!(restored, skill_dir);
    assert!(skill_dir.join("extra.txt").is_file());
    assert!(trash::list_trash_in(&trash_root).is_empty());
}

#[test]
fn restore_refuses_when_target_occupied() {
    let (_home, skill_dir) = make_home_with_skill();
    let data = tmp();
    let trash_root = data.path().join("trash");
    let item = trash::trash_dir_in(&trash_root, &skill_dir, "claude-code").unwrap();

    // 原位置重建了同名目录
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "recreated").unwrap();

    let r = trash::restore_trash_in(&trash_root, &item.id);
    assert!(r.is_err(), "原位置被占用必须拒绝");
    assert!(skill_dir.join("SKILL.md").is_file(), "重建内容不受影响");
    assert_eq!(trash::list_trash_in(&trash_root).len(), 1, "回收站条目保留");
}

#[test]
fn remove_unknown_skill_errors() {
    let (home, _dir) = make_home_with_skill();
    let data = tmp();
    let reg = Registry::with_base(home.path().to_path_buf());
    let r = reg.remove_skill_in(&data.path().join("trash"), "claude-code", "ghost", "user");
    assert!(r.is_err());
}

#[test]
fn snapshot_prune_keeps_newest() {
    let data = tmp();
    let root = data.path().join("snapshots");
    for ms in [1000u64, 2000, 3000, 4000, 5000] {
        let d = root.join(ms.to_string());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("cfg.json"), "x").unwrap();
    }
    let removed = agenthub_core::snapshot::prune_in(&data.path(), 3).unwrap();
    assert_eq!(removed, 2);
    let remaining = agenthub_core::snapshot::prune_in(&data.path(), 3).unwrap();
    assert_eq!(remaining, 0, "再次清理应无可删");
    let left: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(left.len(), 3);
    assert!(left.contains(&"5000".to_string()), "最新组必须保留");
}
