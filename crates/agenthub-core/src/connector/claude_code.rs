use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::model::{AgentDescriptor, McpEntry, McpServerDef};
use crate::registry;

use super::json_config::{def_to_json, entries_from_map, obj_at, read_json, write_json_preserving};
use super::{count_skills, detect_by_paths, Connector, WriteReport};

/// Claude Code：`~/.claude.json`（严格 JSON）。
/// 全局 MCP 在顶层 `mcpServers`，项目级在各 `projects.<路径>.mcpServers`。
pub struct ClaudeCodeConnector {
    desc: AgentDescriptor,
    base: PathBuf,
}

impl ClaudeCodeConnector {
    pub fn from_home() -> Self {
        Self::new(crate::util::home_base())
    }
    pub fn new(base: PathBuf) -> Self {
        Self {
            desc: registry::descriptor("claude-code"),
            base,
        }
    }
    fn config_path(&self) -> PathBuf {
        self.base.join(".claude.json")
    }
}

impl Connector for ClaudeCodeConnector {
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
        let mut out = vec![];
        if let Some(servers) = root.get("mcpServers").and_then(|v| v.as_object()) {            out.extend(entries_from_map("claude-code", "global", servers));
        }
        if let Some(projects) = root.get("projects").and_then(|v| v.as_object()) {
            for (proj, obj) in projects {
                if let Some(servers) = obj.get("mcpServers").and_then(|v| v.as_object()) {
                    out.extend(entries_from_map(
                        "claude-code",
                        &format!("project:{proj}"),
                        servers,
                    ));
                }
            }
        }
        Ok(out)
    }

    fn upsert_mcp(&self, name: &str, def: &McpServerDef) -> Result<WriteReport> {
        let path = self.config_path();
        if !path.exists() {
            return Err(CoreError::NotFound(path.display().to_string()));
        }
        let mut root = read_json(&path)?;
        obj_at(&mut root, &["mcpServers"])?.insert(name.into(), def_to_json(def));
        write_json_preserving(&path, &root)
    }

    fn remove_mcp(&self, name: &str, scope: &str) -> Result<WriteReport> {
        let path = self.config_path();
        if !path.exists() {
            return Err(CoreError::NotFound(path.display().to_string()));
        }
        let mut root = read_json(&path)?;
        let removed = if scope == "global" {
            obj_at(&mut root, &["mcpServers"])?.remove(name).is_some()
        } else if let Some(proj) = scope.strip_prefix("project:") {
            let mut found = false;
            if let Some(projects) = root.get_mut("projects").and_then(|v| v.as_object_mut()) {
                if let Some(obj) = projects.get_mut(proj) {
                    if let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut())
                    {
                        found = servers.remove(name).is_some();
                    }
                }
            }
            found
        } else {
            return Err(CoreError::Other(format!("未知作用域: {scope}")));
        };
        if !removed {
            return Err(CoreError::NotFound(format!("{name} @ {scope}")));
        }
        write_json_preserving(&path, &root)
    }
}
