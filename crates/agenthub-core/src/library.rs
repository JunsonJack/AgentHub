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

/// 从本地路径导入 skill：目录直接 adopt；`.zip` 解压后定位 skill 根再 adopt。
/// zip 结构支持：根层 SKILL.md、唯一子目录含 SKILL.md、或 `name/SKILL.md`。
pub fn import_skill_path(source: &Path, source_tag: &str, dry_run: bool) -> Result<AdoptReport> {
    import_skill_path_into(&app_data_dir(), source, source_tag, dry_run)
}

pub fn import_skill_path_into(
    data_root: &Path,
    source: &Path,
    source_tag: &str,
    dry_run: bool,
) -> Result<AdoptReport> {
    if !source.exists() {
        return Err(CoreError::NotFound(source.display().to_string()));
    }
    let is_zip = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    if !is_zip {
        return adopt_into(data_root, source, source_tag, dry_run);
    }

    let tmp = tempfile_dir()?;
    // 解压到 tmp/<zip 去扩展名>/，使根层 SKILL.md 的 skill 名 = zip 文件名
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "imported-skill".into());
    let extract_root = tmp.join(&stem);
    std::fs::create_dir_all(&extract_root)?;
    extract_zip(source, &extract_root)?;
    let skill_root = find_skill_root(&extract_root)?;
    let report = adopt_into(data_root, &skill_root, source_tag, dry_run)?;
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(report)
}

fn tempfile_dir() -> Result<PathBuf> {
    // 进程内自增 + 毫秒，避免并行导入/测试撞同一临时目录
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "agenthub-zip-{}-{}-{}",
        std::process::id(),
        now_millis(),
        n
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(zip_path)
        .map_err(|e| CoreError::Other(format!("打开 zip 失败: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| CoreError::Other(format!("解析 zip 失败: {e}")))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| CoreError::Other(format!("读取 zip 条目失败: {e}")))?;
        let Some(rel) = entry.enclosed_name() else {
            continue; // 跳过路径穿越
        };
        let out = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buf)?;
        std::fs::write(&out, buf)?;
    }
    Ok(())
}

/// 在解压目录中定位 skill 根：优先根层 SKILL.md，其次唯一子目录/直接子 skill。
fn find_skill_root(extracted: &Path) -> Result<PathBuf> {
    if extracted.join("SKILL.md").is_file() {
        return Ok(extracted.to_path_buf());
    }
    let dirs: Vec<PathBuf> = std::fs::read_dir(extracted)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    // 直接含 SKILL.md 的子目录
    let with_md: Vec<&PathBuf> = dirs.iter().filter(|d| d.join("SKILL.md").is_file()).collect();
    if with_md.len() == 1 {
        return Ok(with_md[0].clone());
    }
    if with_md.len() > 1 {
        return Err(CoreError::Other(format!(
            "zip 内发现多个 skill（各自含 SKILL.md）：{}。请拆分后单独导入",
            with_md
                .iter()
                .filter_map(|d| d.file_name())
                .map(|n| n.to_string_lossy())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    // 唯一子目录时继续下钻一层（常见 GitHub 打包）
    if dirs.len() == 1 {
        let inner = find_skill_root(&dirs[0]);
        if inner.is_ok() {
            return inner;
        }
    }
    Err(CoreError::Other(
        "zip 内未找到 SKILL.md（根层或一级子目录）".into(),
    ))
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
