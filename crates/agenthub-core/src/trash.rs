//! 回收站：skill 删除先移入 <DataDir>/trash/<id>/，可一键恢复（绝不直接销毁用户文件）。

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::util::app_data_dir;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TrashManifest {
    original_path: String,
    agent_id: String,
    name: String,
    deleted_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: String,
    pub original_path: String,
    pub agent_id: String,
    pub name: String,
    pub deleted_at: u64,
}

pub fn trash_root() -> PathBuf {
    app_data_dir().join("trash")
}

/// 把一个 skill 目录移入回收站（同盘 rename，跨盘 copy+delete）
pub fn trash_dir(original_dir: &Path, agent_id: &str) -> Result<TrashItem> {
    trash_dir_in(&trash_root(), original_dir, agent_id)
}

pub fn trash_dir_in(root: &Path, original_dir: &Path, agent_id: &str) -> Result<TrashItem> {
    if !original_dir.is_dir() {
        return Err(CoreError::NotFound(original_dir.display().to_string()));
    }
    let name = original_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| CoreError::Other(format!("非法路径: {}", original_dir.display())))?;
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".into());
    let dest = root.join(&id);
    std::fs::create_dir_all(&dest)?;
    move_dir(original_dir, &dest.join("data"))?;

    let manifest = TrashManifest {
        original_path: original_dir.display().to_string(),
        agent_id: agent_id.into(),
        name: name.clone(),
        deleted_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    };
    std::fs::write(
        dest.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(TrashItem {
        id,
        original_path: manifest.original_path,
        agent_id: agent_id.into(),
        name,
        deleted_at: manifest.deleted_at,
    })
}

pub fn list_trash() -> Vec<TrashItem> {
    list_trash_in(&trash_root())
}

pub fn list_trash_in(root: &Path) -> Vec<TrashItem> {
    let mut out = vec![];
    let Ok(dirs) = std::fs::read_dir(root) else {
        return out;
    };
    for d in dirs.filter_map(|d| d.ok()) {
        let manifest_path = d.path().join("manifest.json");
        if let Ok(text) = std::fs::read_to_string(&manifest_path) {
            if let Ok(m) = serde_json::from_str::<TrashManifest>(&text) {
                out.push(TrashItem {
                    id: d.file_name().to_string_lossy().to_string(),
                    original_path: m.original_path,
                    agent_id: m.agent_id,
                    name: m.name,
                    deleted_at: m.deleted_at,
                });
            }
        }
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

/// 恢复：移回原位置。原位置已被占用（重建了同名 skill）时拒绝，避免覆盖。
pub fn restore_trash_in(root: &Path, id: &str) -> Result<PathBuf> {
    let item_dir = root.join(id);
    let manifest_path = item_dir.join("manifest.json");
    let manifest: TrashManifest = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path)
            .map_err(|_| CoreError::NotFound(format!("回收站条目 {id} 不存在或已损坏")))?,
    )?;
    let target = PathBuf::from(&manifest.original_path);
    if target.exists() {
        return Err(CoreError::Other(format!(
            "原位置已存在同名目录，拒绝覆盖：{}",
            target.display()
        )));
    }
    move_dir(&item_dir.join("data"), &target)?;
    let _ = std::fs::remove_dir_all(&item_dir);
    Ok(target)
}

pub fn restore_trash(id: &str) -> Result<PathBuf> {
    restore_trash_in(&trash_root(), id)
}

fn move_dir(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    // 跨盘回退：复制 + 删除
    crate::library::copy_dir_all(src, dest)?;
    std::fs::remove_dir_all(src)?;
    Ok(())
}
