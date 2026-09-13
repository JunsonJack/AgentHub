use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{CoreError, Result};
use crate::util::app_data_dir;

/// 任何写配置动作前自动快照（规划 P0 安全网）。
/// 备份布局：<DataDir>/snapshots/<unix 毫秒>/<原文件名>
pub fn snapshot_file(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Err(CoreError::NotFound(path.display().to_string()));
    }
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dir = app_data_dir().join("snapshots").join(ms.to_string());
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(
        path.file_name()
            .ok_or_else(|| CoreError::Other(format!("非法路径: {}", path.display())))?,
    );
    std::fs::copy(path, &dest)?;
    Ok(dest)
}
