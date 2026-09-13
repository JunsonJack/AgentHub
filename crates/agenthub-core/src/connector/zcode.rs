use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::model::{AgentDescriptor, McpEntry, McpServerDef};
use crate::registry;

use super::json_config::{def_to_json, entries_from_map, obj_at, read_json, write_json_preserving};
use super::{count_skills, detect_by_paths, Connector, WriteReport};

/// ZCode：`~/.zcode/cli/config.json`，MCP 在 `mcp.servers.<名称>`（名称可含中文）。
pub struct ZcodeConnector {
    desc: AgentDescriptor,
    base: PathBuf,
}

impl ZcodeConnector {
    pub fn from_home() -> Self {
        Self::new(crate::util::home_base())
    }
    pub fn new(base: PathBuf) -> Self {
        Self {
            desc: registry::descriptor("zcode"),
            base,
        }
    }
    fn config_path(&self) -> PathBuf {
        self.base.join(".zcode").join("cli").join("config.json")
    }
}

impl Connector for ZcodeConnector {
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
        let root = read_json(&path)?;
        let empty = serde_json::Map::new();
        let servers = root
            .get("mcp")
            .and_then(|m| m.get("servers"))
            .and_then(|s| s.as_object())
            .unwrap_or(&empty);
        Ok(entries_from_map("zcode", "global", servers))
    }

    fn upsert_mcp(&self, name: &str, def: &McpServerDef) -> Result<WriteReport> {
        let path = self.config_path();
        if !path.exists() {
            return Err(CoreError::NotFound(path.display().to_string()));
        }
        let mut root = read_json(&path)?;
        obj_at(&mut root, &["mcp", "servers"])?.insert(name.into(), def_to_json(def));
        write_json_preserving(&path, &root)
    }

    fn remove_mcp(&self, name: &str, scope: &str) -> Result<WriteReport> {
        if scope != "global" {
            return Err(CoreError::Other(format!("未知作用域: {scope}")));
        }
        let path = self.config_path();
        if !path.exists() {
            return Err(CoreError::NotFound(path.display().to_string()));
        }
        let mut root = read_json(&path)?;
        if obj_at(&mut root, &["mcp", "servers"])?
            .remove(name)
            .is_none()
        {
            return Err(CoreError::NotFound(format!("{name} @ {scope}")));
        }
        write_json_preserving(&path, &root)
    }
}
