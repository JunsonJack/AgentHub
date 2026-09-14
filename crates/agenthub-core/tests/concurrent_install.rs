//! 并发安装回归测试。
//!
//! 守住的红线：多个安装 / 更新检查同时进行（市场连点、更新器批量 stage）时，
//! 各自的临时目录不得互相撞名或共用。以前临时目录只用毫秒时间戳命名，
//! 同一毫秒的并发调用会命中同一路径，表现为 Windows 上偶发的
//! PermissionDenied / "already exists and is not an empty directory"。

use std::path::Path;

use agenthub_core::market;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn git(cwd: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git 可用");
    assert!(out.status.success(), "git {:?} 失败: {}", args, String::from_utf8_lossy(&out.stderr));
}

/// 建一个最小可克隆的上游仓库（离线）
fn make_upstream(root: &Path, name: &str) -> String {
    let repo = root.join(name);
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: 并发测试用\n---\n\n正文\n"),
    )
    .unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["-c", "user.name=t", "-c", "user.email=t@t.t", "add", "-A"]);
    git(&repo, &["-c", "user.name=t", "-c", "user.email=t@t.t", "commit", "-q", "-m", "init"]);
    repo.to_string_lossy().replace('\\', "/")
}

#[test]
fn concurrent_git_installs_do_not_collide() {
    const THREADS: usize = 8;

    let upstream_root = tmp();
    let url = make_upstream(upstream_root.path(), "conc");

    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let url = url.clone();
            std::thread::spawn(move || {
                let data = tmp();
                let outcome = (|| {
                    let report = market::install_git(&url, data.path(), false)?;
                    // 入库内容必须完整，不能被别的线程的快照污染
                    let md = std::fs::read_to_string(
                        agenthub_core::library::skills_root(data.path())
                            .join(&report.skill_name)
                            .join("SKILL.md"),
                    )?;
                    Ok::<_, agenthub_core::error::CoreError>(md.contains("name: conc"))
                })();
                (data, outcome)
            })
        })
        .collect();

    let mut oks = 0;
    let mut errs = vec![];
    for h in handles {
        // _keep 把 TempDir 撑到本轮断言之后，避免目录提前被删
        let (_keep, outcome) = h.join().expect("线程不得 panic");
        match outcome {
            Ok(true) => oks += 1,
            Ok(false) => errs.push("内容校验失败".to_string()),
            Err(e) => errs.push(e.to_string()),
        }
    }
    assert_eq!(
        oks, THREADS,
        "{THREADS} 个并发安装应全部成功，实际 {oks} 成功；报错样本：{:?}",
        errs.iter().take(3).collect::<Vec<_>>()
    );
}

#[test]
fn concurrent_fetch_temp_roots_are_distinct() {
    let upstream_root = tmp();
    let url = make_upstream(upstream_root.path(), "distinct");

    let handles: Vec<_> = (0..6)
        .map(|_| {
            let url = url.clone();
            std::thread::spawn(move || {
                let f = market::fetch_git_to_temp(&url).expect("fetch 不得因目录冲突失败");
                let root = f.cleanup_root.clone();
                let _ = std::fs::remove_dir_all(&f.cleanup_root);
                root
            })
        })
        .collect();

    let roots: Vec<String> = handles
        .into_iter()
        .map(|h| h.join().unwrap().to_string_lossy().to_string())
        .collect();
    let unique: std::collections::HashSet<&String> = roots.iter().collect();
    assert_eq!(
        unique.len(),
        roots.len(),
        "临时克隆根目录必须两两不同，实际：{roots:?}"
    );
}
