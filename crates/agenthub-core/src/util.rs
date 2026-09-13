use std::path::{Path, PathBuf};

use crate::model::OsPaths;

/// 展开注册表路径：`~/x` → base/x。生产环境 base = HOME，测试时指向临时目录。
pub fn resolve(p: &str, base: &Path) -> PathBuf {
    let s = p.strip_prefix("~/").unwrap_or(p);
    base.join(s)
}

pub fn pick_os(paths: &OsPaths) -> &[String] {
    if cfg!(target_os = "windows") {
        &paths.windows
    } else if cfg!(target_os = "macos") {
        &paths.macos
    } else {
        &paths.linux
    }
}

pub fn resolve_all(paths: &OsPaths, base: &Path) -> Vec<PathBuf> {
    pick_os(paths).iter().map(|p| resolve(p, base)).collect()
}

pub fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("AgentHub")
}

/// 连接器基准目录（生产 = HOME；测试用临时目录替代）
pub fn home_base() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// 连接器的主配置路径：注册表（含覆写）声明的当前 OS 第一条路径
pub fn primary_config_path(desc: &crate::model::AgentDescriptor, base: &Path, fallback: &str) -> PathBuf {
    let resolved = resolve_all(&desc.mcp_config_paths, base);
    resolved.into_iter().next().unwrap_or_else(|| base.join(fallback))
}
