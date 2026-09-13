use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::model::{AgentDescriptor, McpEntry, McpServerDef};
use crate::registry;

use super::json_config::{entries_from_map, read_json};
use super::{count_skills, detect_by_paths, Connector, WriteReport};

/// Cursor：`~/.cursor/mcp.json`（官方示例带注释，JSONC 格式待 W0 实测核验）。
/// v0 只做探测与严格 JSON 读取；写入留到 JSONC 行为核验后实现。
pub struct CursorConnector {
    desc: AgentDescriptor,
    base: PathBuf,
}

impl CursorConnector {
    pub fn from_home() -> Self {
        Self::new(crate::util::home_base())
    }
    pub fn new(base: PathBuf) -> Self {
        Self {
            desc: registry::descriptor("cursor"),
            base,
        }
    }
    fn config_path(&self) -> PathBuf {
        self.base.join(".cursor").join("mcp.json")
    }
}

impl Connector for CursorConnector {
    fn descriptor(&self) -> &AgentDescriptor {
        &self.desc
    }
    fn base_dir(&self) -> &Path {
        &self.base
    }

    fn detect(&self) -> Result<crate::model::AgentStatus> {
        let mut st = detect_by_paths(&self.desc, &self.base)?;
        st.skill_count = count_skills(&self.desc.skill_dirs, &self.base);
        Ok(st)
    }

    fn list_mcp(&self) -> Result<Vec<McpEntry>> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(vec![]);
        }
        let root = read_json(&path).map_err(|e| {
            CoreError::Other(format!(
                "{} 无法按严格 JSON 解析（疑似 JSONC，W0 待核验）: {e}",
                path.display()
            ))
        })?;
        let empty = serde_json::Map::new();
        let servers = root
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty);
        Ok(entries_from_map("cursor", "global", servers))
    }

    fn upsert_mcp(&self, _name: &str, _def: &McpServerDef) -> Result<WriteReport> {
        Err(CoreError::Unsupported(
            "Cursor 写入待 JSONC 行为核验后实现（W0）".into(),
        ))
    }
}
