use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::model::{AgentDescriptor, McpEntry, McpServerDef};
use crate::snapshot;
use crate::util::resolve_all;

use super::json_config::{def_to_json, detect_transport, read_json, write_json_preserving};
use super::{Connector, WriteReport};

/// dsh 的 MCP 配置是 { version, servers: [{ name, ... }] }。
/// 除 MCP 标准字段外，enabled/cwd/reconnect/toolCallTimeoutMs 等全部原样保留。
pub struct DshConnector {
    desc: AgentDescriptor,
    base: PathBuf,
}

impl DshConnector {
    pub fn with_descriptor(desc: AgentDescriptor, base: PathBuf) -> Self { Self { desc, base } }

    fn config_path(&self) -> Option<PathBuf> {
        resolve_all(&self.desc.mcp_config_paths, &self.base).into_iter().find(|p| p.exists())
    }

    fn path_or_not_found(&self) -> Result<PathBuf> {
        self.config_path().ok_or_else(|| CoreError::NotFound(self.desc.name.clone()))
    }
}

impl Connector for DshConnector {
    fn descriptor(&self) -> &AgentDescriptor { &self.desc }
    fn base_dir(&self) -> &Path { &self.base }

    fn list_mcp(&self) -> Result<Vec<McpEntry>> {
        let Some(path) = self.config_path() else { return Ok(vec![]) };
        let root = read_json(&path)?;
        Ok(entries_from_dsh(&self.desc.id, "global", root.get("servers").and_then(|v| v.as_array()).map(Vec::as_slice).unwrap_or(&[])))
    }

    fn upsert_mcp(&self, name: &str, def: &McpServerDef, scope: &str) -> Result<WriteReport> {
        if scope != "global" && !scope.is_empty() {
            return Err(CoreError::Other(format!("未知作用域: {scope}")));
        }
        let path = self.path_or_not_found()?;
        let mut root = read_json(&path)?;
        let servers = root.get_mut("servers").and_then(|v| v.as_array_mut())
            .ok_or_else(|| CoreError::Other("dsh 配置缺少 servers 数组".into()))?;
        let mut new_def = def_to_json(def);
        let obj = new_def.as_object_mut().expect("McpServerDef serializes to object");
        obj.insert("name".into(), name.into());
        if let Some(existing) = servers.iter_mut().find(|v| v.get("name").and_then(|n| n.as_str()) == Some(name)) {
            let old = existing.as_object().cloned().unwrap_or_default();
            let next = obj.clone();
            let mut merged = old;
            for (k, v) in next { merged.insert(k, v); }
            *existing = serde_json::Value::Object(merged);
        } else {
            servers.push(new_def);
        }
        write_json_preserving(&path, &root)
    }

    fn remove_mcp(&self, name: &str, scope: &str) -> Result<WriteReport> {
        if scope != "global" { return Err(CoreError::Other(format!("未知作用域: {scope}"))); }
        let path = self.path_or_not_found()?;
        let mut root = read_json(&path)?;
        let servers = root.get_mut("servers").and_then(|v| v.as_array_mut())
            .ok_or_else(|| CoreError::Other("dsh 配置缺少 servers 数组".into()))?;
        let before = servers.len();
        servers.retain(|v| v.get("name").and_then(|n| n.as_str()) != Some(name));
        if servers.len() == before { return Err(CoreError::NotFound(name.into())); }
        write_json_preserving(&path, &root)
    }
}

/// 将 dsh servers 数组转换成统一 MCP 条目。数组元素里的 name 不属于 def 本身。
pub fn entries_from_dsh(agent_id: &str, scope: &str, servers: &[serde_json::Value]) -> Vec<McpEntry> {
    servers.iter().filter_map(|v| {
        let name = v.get("name")?.as_str()?.to_string();
        let raw = v.as_object().cloned().unwrap_or_default();
        let mut def = raw.clone();
        def.remove("name");
        let transport = detect_transport(&def);
        Some(McpEntry {
            agent_id: agent_id.into(), name, scope: scope.into(), transport,
            command: def.get("command").and_then(|v| v.as_str()).map(String::from),
            args: def.get("args").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
            url: def.get("url").and_then(|v| v.as_str()).map(String::from),
            raw: serde_json::Value::Object(raw),
        })
    }).collect()
}

/// dsh 不使用公共 map connector；保留一个可测试的快照写入入口。
pub fn write_dsh(path: &Path, root: &serde_json::Value) -> Result<WriteReport> {
    let backup = snapshot::snapshot_file(path)?;
    let mut text = serde_json::to_string_pretty(root)?;
    text.push('\n');
    std::fs::write(path, text)?;
    Ok(WriteReport { backup_path: Some(backup.display().to_string()), changed: true })
}
