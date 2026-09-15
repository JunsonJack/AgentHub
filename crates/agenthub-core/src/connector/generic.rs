use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::model::{AgentDescriptor, McpEntry, McpServerDef};
use crate::registry;
use crate::util::resolve_all;

use super::json_config::{entries_from_map, read_json};
use super::{count_skills, Connector, WriteReport};
use super::{claude_code::ClaudeCodeConnector, codex::CodexConnector, cursor::CursorConnector, zcode::ZcodeConnector};

/// 通用 JSON MCP 连接器（Claude Desktop / Gemini CLI）：
/// 探测注册表里第一个存在的配置路径，读取顶层 `mcpServers`。
pub struct GenericJsonMcpConnector {
    desc: AgentDescriptor,
    base: PathBuf,
}

impl GenericJsonMcpConnector {
    pub fn new(id: &str, base: PathBuf) -> Self {
        Self::with_descriptor(registry::descriptor(id), base)
    }
    pub fn with_descriptor(desc: AgentDescriptor, base: PathBuf) -> Self {
        Self { desc, base }
    }
    fn config_path(&self) -> Option<PathBuf> {
        resolve_all(&self.desc.mcp_config_paths, &self.base)
            .into_iter()
            .find(|p| p.exists())
    }
}

impl Connector for GenericJsonMcpConnector {
    fn descriptor(&self) -> &AgentDescriptor {
        &self.desc
    }
    fn base_dir(&self) -> &Path {
        &self.base
    }

    fn list_mcp(&self) -> Result<Vec<McpEntry>> {
        let Some(path) = self.config_path() else {
            return Ok(vec![]);
        };
        let root = read_json(&path)?;
        let empty = serde_json::Map::new();
        let servers = root
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty);
        Ok(entries_from_map(&self.desc.id, "global", servers))
    }

    fn upsert_mcp(&self, name: &str, def: &McpServerDef, scope: &str) -> Result<WriteReport> {
        if scope != "global" && !scope.is_empty() {
            return Err(crate::error::CoreError::Other(format!("未知作用域: {scope}")));
        }
        let Some(path) = self.config_path() else {
            return Err(crate::error::CoreError::NotFound(
                self.desc.name.clone(),
            ));
        };
        let mut root = read_json(&path)?;
        super::json_config::obj_at(&mut root, &["mcpServers"])?
            .insert(name.to_string(), super::json_config::def_to_json(def));
        super::json_config::write_json_preserving(&path, &root)
    }

    fn remove_mcp(&self, name: &str, scope: &str) -> Result<WriteReport> {
        if scope != "global" {
            return Err(crate::error::CoreError::Other(format!("未知作用域: {scope}")));
        }
        let Some(path) = self.config_path() else {
            return Err(crate::error::CoreError::NotFound(
                self.desc.name.clone(),
            ));
        };
        let mut root = read_json(&path)?;
        if super::json_config::obj_at(&mut root, &["mcpServers"])?
            .remove(name)
            .is_none()
        {
            return Err(crate::error::CoreError::NotFound(name.to_string()));
        }
        super::json_config::write_json_preserving(&path, &root)
    }
}

/// 兜底：仅探测，不解析（新 Agent 接入前的占位）
pub struct DetectOnlyConnector {
    desc: AgentDescriptor,
    base: PathBuf,
}

impl DetectOnlyConnector {
    pub fn new(id: &str, base: PathBuf) -> Self {
        Self::with_descriptor(registry::descriptor(id), base)
    }
    pub fn with_descriptor(desc: AgentDescriptor, base: PathBuf) -> Self {
        Self { desc, base }
    }
}

impl Connector for DetectOnlyConnector {
    fn descriptor(&self) -> &AgentDescriptor {
        &self.desc
    }
    fn base_dir(&self) -> &Path {
        &self.base
    }
    fn list_mcp(&self) -> Result<Vec<McpEntry>> {
        Ok(vec![])
    }
    fn upsert_mcp(&self, _n: &str, _d: &McpServerDef, _scope: &str) -> Result<WriteReport> {
        Err(crate::error::CoreError::Unsupported("仅探测连接器".into()))
    }
}

/// 供注册表构造使用：按描述符生成对应连接器
pub fn make_connector(desc: &AgentDescriptor, base: PathBuf) -> Box<dyn Connector> {
    // 自定义 Agent 也能通过 mcpFormat 选择适配器；不要把格式路由绑定在 id 上。
    if desc.mcp_format == "dsh-array" {
        return Box::new(super::dsh::DshConnector::with_descriptor(desc.clone(), base));
    }
    match desc.id.as_str() {
        "claude-code" => Box::new(ClaudeCodeConnector::with_descriptor(desc.clone(), base)),
        "zcode" => Box::new(ZcodeConnector::with_descriptor(desc.clone(), base)),
        "codex" => Box::new(CodexConnector::with_descriptor(desc.clone(), base)),
        "cursor" => Box::new(CursorConnector::with_descriptor(desc.clone(), base)),
        "dsh" => Box::new(super::dsh::DshConnector::with_descriptor(desc.clone(), base)),
        "gemini-cli" | "claude-desktop" | "pi" => {
            Box::new(GenericJsonMcpConnector::with_descriptor(desc.clone(), base))
        }
        _ => Box::new(GenericJsonMcpConnector::with_descriptor(desc.clone(), base)),
    }
}

/// detect() 结果包装（registry.statuses 用）
pub fn status_of(c: &dyn Connector) -> crate::model::AgentStatus {
    let desc = c.descriptor().clone();
    match c.detect() {
        Ok(mut st) => {
            match c.list_mcp() {
                Ok(v) => st.mcp_count = Some(v.len()),
                Err(e) => {
                    // 解析失败属于健康度问题，直接浮出到仪表盘
                    st.health_note = Some(format!("MCP 配置解析失败：{e}"));
                }
            }
            if st.installed && st.skill_count.is_none() {
                st.skill_count = count_skills(&desc.skill_dirs, c.base_dir());
            }
            st
        }
        Err(e) => crate::model::AgentStatus {
            id: desc.id,
            name: desc.name,
            kind: desc.kind,
            installed: false,
            found_paths: vec![],
            mcp_count: None,
            skill_count: None,
            health_note: Some(e.to_string()),
        },
    }
}
