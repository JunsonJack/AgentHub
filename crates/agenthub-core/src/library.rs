//! 中央库（Library）：所有收编的 skill 的单一事实来源。
//! 文件形态存储（<DataDir>/library/skills/<name>/），每个条目带 manifest.json，
//! 便于 P2 git 化与社区分享。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::model::{AdoptReport, LibraryItem};
use crate::util::app_data_dir;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[serde(flatten)]
    item: LibraryItem,
}

/// collect_files 供外部统计文件数（manifest 重写用）
pub(crate) fn count_files(dir: &Path) -> Result<usize> {
    let mut out = vec![];
    collect_files(dir, dir, &mut out)?;
    Ok(out.len())
}

pub fn library_dir() -> PathBuf {
    skills_root(&app_data_dir())
}

pub fn skills_root(data_root: &Path) -> PathBuf {
    data_root.join("library").join("skills")
}

/// 收编：把 Agent 目录里的存量 skill 拷入中央库。
/// 语义（借鉴 skills-manager）：目标已存在 = 冲突，拒绝覆盖，绝不代删。
pub fn adopt(source_dir: &Path, source_agent: &str, dry_run: bool) -> Result<AdoptReport> {
    adopt_into(&app_data_dir(), source_dir, source_agent, dry_run)
}

pub fn adopt_into(
    data_root: &Path,
    source_dir: &Path,
    source_agent: &str,
    dry_run: bool,
) -> Result<AdoptReport> {
    if !source_dir.is_dir() {
        return Err(CoreError::NotFound(source_dir.display().to_string()));
    }
    let name = source_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| CoreError::Other(format!("非法目录: {}", source_dir.display())))?;
    let target = skills_root(data_root).join(&name);
    let conflict = target.exists();

    let mut files = Vec::new();
    collect_files(source_dir, source_dir, &mut files)?;

    if !dry_run && !conflict {
        copy_rec(source_dir, &target)?;
        let (_fm_name, description) = crate::connector::parse_frontmatter(&target.join("SKILL.md"));
        let item = LibraryItem {
            // 目录名是规范标识；frontmatter 名只作展示参考
            name: name.clone(),
            kind: "skill".into(),
            source_agent: Some(source_agent.into()),
            source_path: Some(source_dir.display().to_string()),
            description,
            adopted_at: now_millis(),
            file_count: files.len(),
            updated_at: None,
        };
        let manifest = serde_json::to_string_pretty(&Manifest { item })?;
        std::fs::write(target.join("manifest.json"), manifest)?;
    }

    Ok(AdoptReport {
        dry_run,
        skill_name: name,
        target_dir: target.display().to_string(),
        files,
        conflict,
    })
}

pub fn list_library() -> Vec<LibraryItem> {
    list_library_in(&app_data_dir())
}

pub fn list_library_in(data_root: &Path) -> Vec<LibraryItem> {
    let mut out = vec![];
    let Ok(entries) = std::fs::read_dir(skills_root(data_root)) else {
        return out;
    };
    for e in entries.filter_map(|e| e.ok()) {
        let manifest = e.path().join("manifest.json");
        if !manifest.is_file() {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&manifest) {
            if let Ok(m) = serde_json::from_str::<Manifest>(&text) {
                out.push(m.item);
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 读取某个库内条目的 SKILL.md 内容（详情抽屉渲染用）
pub fn read_library_skill_md(name: &str) -> Result<String> {
    read_skill_md_in(&app_data_dir(), name)
}

pub fn read_skill_md_in(data_root: &Path, name: &str) -> Result<String> {
    let path = skills_root(data_root).join(name).join("SKILL.md");
    if !path.is_file() {
        return Ok(String::new());
    }
    Ok(std::fs::read_to_string(path)?)
}

/// 读取条目 manifest（更新器需要 source 字段）
pub(crate) fn read_manifest(data_root: &Path, name: &str) -> Result<LibraryItem> {
    let manifest = skills_root(data_root).join(name).join("manifest.json");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|_| CoreError::NotFound(manifest.display().to_string()))?;
    serde_json::from_str::<Manifest>(&text)
        .map(|m| m.item)
        .map_err(Into::into)
}

/// 直接写入 manifest（更新流程在删除旧内容前先把 item 读出来，拷完再写回）
pub(crate) fn write_manifest_item(data_root: &Path, name: &str, item: &LibraryItem) -> Result<()> {
    let manifest = serde_json::to_string_pretty(&Manifest { item: item.clone() })?;
    std::fs::write(skills_root(data_root).join(name).join("manifest.json"), manifest)?;
    Ok(())
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for e in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            collect_files(root, &p, out)?;
        } else {
            let rel = p
                .strip_prefix(root)
                .map_err(|e| CoreError::Other(e.to_string()))?
                .to_string_lossy()
                .to_string();
            out.push(rel.replace('\\', "/"));
        }
    }
    Ok(())
}

/// 递归复制目录（deploy_skill 与测试共用）
pub fn copy_dir_all(src: &Path, dest: &Path) -> Result<()> {
    copy_rec(src, dest)
}

fn copy_rec(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for e in std::fs::read_dir(src)?.filter_map(|e| e.ok()) {
        let p = e.path();
        let target = dest.join(e.file_name());
        if p.is_dir() {
            copy_rec(&p, &target)?;
        } else {
            std::fs::copy(&p, &target)?;
        }
    }
    Ok(())
}
