pub mod claude_code;
pub mod codex;
pub mod cursor;
pub mod generic;
pub mod json_config;
pub mod zcode;

use std::path::Path;

use crate::error::{CoreError, Result};
use crate::model::{AgentDescriptor, AgentStatus, McpEntry, McpServerDef, OsPaths};
use crate::util::resolve_all;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteReport {
    pub backup_path: Option<String>,
    pub changed: bool,
}

pub trait Connector: Send + Sync {
    fn descriptor(&self) -> &AgentDescriptor;

    /// 连接器工作的基准目录（生产 = HOME，测试 = 临时目录）
    fn base_dir(&self) -> &Path;

    fn detect(&self) -> Result<AgentStatus> {
        detect_by_paths(self.descriptor(), self.base_dir())
    }

    fn list_mcp(&self) -> Result<Vec<McpEntry>>;

    fn upsert_mcp(&self, _name: &str, _def: &McpServerDef) -> Result<WriteReport> {
        Err(CoreError::Unsupported(format!(
            "{}: MCP 写入暂未实现",
            self.descriptor().name
        )))
    }

    fn remove_mcp(&self, _name: &str, _scope: &str) -> Result<WriteReport> {
        Err(CoreError::Unsupported(format!(
            "{}: MCP 删除暂未实现",
            self.descriptor().name
        )))
    }
}

/// 默认探测：任一配置路径或 skill 目录存在即视为已安装
pub fn detect_by_paths(desc: &AgentDescriptor, base: &Path) -> Result<AgentStatus> {
    let mut found = Vec::new();
    for p in resolve_all(&desc.mcp_config_paths, base) {
        if p.exists() {
            found.push(p.display().to_string());
        }
    }
    for p in resolve_all(&desc.skill_dirs, base) {
        if p.exists() {
            found.push(p.display().to_string());
        }
    }
    Ok(AgentStatus {
        id: desc.id.clone(),
        name: desc.name.clone(),
        kind: desc.kind,
        installed: !found.is_empty(),
        found_paths: found,
        mcp_count: None,
        skill_count: None,
        health_note: None,
    })
}

/// 统计 skill 目录下的子目录数（SKILL.md 完整性校验留给下一步）
pub fn count_skills(dirs: &OsPaths, base: &Path) -> Option<usize> {
    let dir = resolve_all(dirs, base).into_iter().find(|p| p.exists())?;
    let n = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .count();
    Some(n)
}
