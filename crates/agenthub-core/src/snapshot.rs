use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::model::SnapshotMeta;
use crate::util::app_data_dir;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotManifest {
    original_path: String,
    created_at: u64,
}

pub fn snapshots_root() -> PathBuf {
    root(&app_data_dir())
}

fn root(data_root: &Path) -> PathBuf {
    data_root.join("snapshots")
}

/// 任何写配置动作前自动快照（规划 P0 安全网）。
/// 布局：<DataDir>/snapshots/<unix 毫秒>/<原文件名> + 同名 .manifest.json（记录原路径）。
pub fn snapshot_file(path: &Path) -> Result<PathBuf> {
    snapshot_in(&app_data_dir(), path)
}

pub fn snapshot_in(data_root: &Path, path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Err(CoreError::NotFound(path.display().to_string()));
    }
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let dir = root(data_root).join(ms.to_string());
    std::fs::create_dir_all(&dir)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| CoreError::Other(format!("非法路径: {}", path.display())))?
        .to_string_lossy()
        .to_string();
    let dest = dir.join(&file_name);
    std::fs::copy(path, &dest)?;
    let manifest = serde_json::to_string_pretty(&SnapshotManifest {
        original_path: path.display().to_string(),
        created_at: ms,
    })?;
    std::fs::write(dir.join(format!("{file_name}.manifest.json")), manifest)?;
    Ok(dest)
}

/// 浏览全部快照（设置页用）
pub fn list_snapshots() -> Vec<SnapshotMeta> {
    list_in(&app_data_dir())
}

pub fn list_in(data_root: &Path) -> Vec<SnapshotMeta> {
    let mut out = vec![];
    let Ok(dirs) = std::fs::read_dir(root(data_root)) else {
        return out;
    };
    let mut dir_infos: Vec<(String, u64)> = dirs
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            let ts = id.parse::<u64>().unwrap_or(0);
            Some((id, ts))
        })
        .collect();
    dir_infos.sort_by(|a, b| b.0.cmp(&a.0));
    for (id, ts) in dir_infos {
        let dir = root(data_root).join(&id);
        let Ok(files) = std::fs::read_dir(&dir) else {
            continue;
        };
        for f in files.filter_map(|f| f.ok()) {
            let name = f.file_name().to_string_lossy().to_string();
            if name.ends_with(".manifest.json") {
                continue;
            }
            let manifest_path = dir.join(format!("{name}.manifest.json"));
            let (original_path, created_at) = std::fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|t| serde_json::from_str::<SnapshotManifest>(&t).ok())
                .map(|m| (m.original_path, m.created_at))
                .unwrap_or_else(|| (String::new(), ts));
            out.push(SnapshotMeta {
                id: id.clone(),
                file_name: name,
                original_path,
                created_at,
            });
        }
    }
    out
}

/// 手动清理旧快照：保留时间戳最新的 `keep` 组，返回删除的组数。
/// 只做显式调用，绝不自动删除（快照是用户的安全网）。
pub fn prune(keep: usize) -> Result<usize> {
    prune_in(&app_data_dir(), keep)
}

pub fn prune_in(data_root: &Path, keep: usize) -> Result<usize> {
    let root = root(data_root);
    let Ok(dirs) = std::fs::read_dir(&root) else {
        return Ok(0);
    };
    let mut names: Vec<(String, u64)> = dirs
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            let ts = id.parse::<u64>().ok()?;
            Some((id, ts))
        })
        .collect();
    names.sort_by(|a, b| b.1.cmp(&a.1));
    let mut removed = 0;
    for (id, _) in names.into_iter().skip(keep) {
        if std::fs::remove_dir_all(root.join(&id)).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// 回滚：把备份拷回原路径。回滚前先对当前文件再拍一次快照（可撤销的回滚）。
pub fn rollback(id: &str, file_name: &str) -> Result<PathBuf> {
    rollback_in(&app_data_dir(), id, file_name)
}

pub fn rollback_in(data_root: &Path, id: &str, file_name: &str) -> Result<PathBuf> {
    let backup = root(data_root).join(id).join(file_name);
    if !backup.is_file() {
        return Err(CoreError::NotFound(backup.display().to_string()));
    }
    let manifest_path = backup.with_file_name(format!("{file_name}.manifest.json"));
    let original: String = if manifest_path.is_file() {
        serde_json::from_str::<SnapshotManifest>(&std::fs::read_to_string(&manifest_path)?)?
            .original_path
    } else {
        return Err(CoreError::Other("快照缺少 manifest，无法确定原路径".into()));
    };
    let target = PathBuf::from(&original);
    if target.exists() {
        snapshot_in(data_root, &target)?;
    } else if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&backup, &target)?;
    Ok(target)
}
