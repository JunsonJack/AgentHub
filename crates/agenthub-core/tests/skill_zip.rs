//! zip 导入到中央库

use agenthub_core::library::{import_skill_path_into, list_library_in, skills_root};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

fn write_skill_zip(zip_path: &Path, entries: &[(&str, &str)]) {
    let file = std::fs::File::create(zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    for (name, content) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn import_zip_with_root_skill_md() {
    let work = tempdir().unwrap();
    let zip_path = work.path().join("my-skill.zip");
    write_skill_zip(
        &zip_path,
        &[
            ("SKILL.md", "---\nname: my-skill\ndescription: demo\n---\nbody"),
            ("scripts/run.sh", "#!/bin/sh\necho hi"),
        ],
    );

    let data = tempdir().unwrap();
    let report = import_skill_path_into(data.path(), &zip_path, "zip", false).unwrap();
    assert!(!report.conflict);
    assert_eq!(report.skill_name, "my-skill");
    assert!(skills_root(data.path()).join("my-skill/SKILL.md").is_file());
    assert!(skills_root(data.path()).join("my-skill/scripts/run.sh").is_file());
}

#[test]
fn import_zip_subdir_skill() {
    let work = tempdir().unwrap();
    let zip_path = work.path().join("pack.zip");
    write_skill_zip(
        &zip_path,
        &[
            ("awesome-skill/SKILL.md", "---\nname: awesome-skill\ndescription: nested\n---\n"),
            ("awesome-skill/lib/util.md", "# util"),
        ],
    );

    let data = tempdir().unwrap();
    let report = import_skill_path_into(data.path(), &zip_path, "zip", false).unwrap();
    assert!(!report.conflict);
    assert_eq!(report.skill_name, "awesome-skill");
    assert!(skills_root(data.path()).join("awesome-skill/SKILL.md").is_file());
    assert!(skills_root(data.path()).join("awesome-skill/lib/util.md").is_file());

    let items = list_library_in(data.path());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "awesome-skill");
    assert!(items[0].file_count >= 2);
}

#[test]
fn import_zip_conflict_refuses_overwrite() {
    let work = tempdir().unwrap();
    let zip_path = work.path().join("pack.zip");
    write_skill_zip(
        &zip_path,
        &[("dup/SKILL.md", "---\nname: dup\n---\n")],
    );
    let data = tempdir().unwrap();
    let r1 = import_skill_path_into(data.path(), &zip_path, "zip", false).unwrap();
    assert!(!r1.conflict);
    let r2 = import_skill_path_into(data.path(), &zip_path, "zip", false).unwrap();
    assert!(r2.conflict);
}

#[test]
fn import_zip_without_skill_md_fails() {
    let work = tempdir().unwrap();
    let zip_path = work.path().join("empty.zip");
    write_skill_zip(&zip_path, &[("readme.txt", "hi")]);
    let data = tempdir().unwrap();
    let err = import_skill_path_into(data.path(), &zip_path, "zip", false).unwrap_err();
    assert!(err.to_string().contains("SKILL.md"));
}

#[test]
fn import_dir_still_works() {
    let work = tempdir().unwrap();
    let skill = work.path().join("folder-skill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "---\nname: folder-skill\n---\n").unwrap();
    let data = tempdir().unwrap();
    let report = import_skill_path_into(data.path(), &skill, "dir", false).unwrap();
    assert_eq!(report.skill_name, "folder-skill");
    assert!(skills_root(data.path()).join("folder-skill/SKILL.md").is_file());
}
