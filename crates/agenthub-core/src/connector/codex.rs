use std::path::{Path, PathBuf};

use toml_edit::{value, DocumentMut, Item, Table};
use toml::Value as TomlValue;

use crate::error::{CoreError, Result};
use crate::model::{AgentDescriptor, McpEntry, McpServerDef};
use crate::registry;
use crate::snapshot;

use super::json_config::detect_transport;
use super::{count_skills, detect_by_paths, Connector, WriteReport};

/// Codex CLI：`~/.codex/config.toml`，MCP 在 `[mcp_servers.<name>]`。
/// 写入走 toml_edit，保留注释 / 键序 / 排版（规划红线）。
pub struct CodexConnector {
    desc: AgentDescriptor,
    base: PathBuf,
}

impl CodexConnector {
    pub fn from_home() -> Self {
        Self::new(crate::util::home_base())
    }
    pub fn new(base: PathBuf) -> Self {
        Self {
            desc: registry::descriptor("codex"),
            base,
        }
    }
    fn config_path(&self) -> PathBuf {
        self.base.join(".codex").join("config.toml")
    }
}

fn toml_table_to_entry(agent_id: &str, name: &str, def: &toml::map::Map<String, TomlValue>) -> McpEntry {
    let raw = serde_json::to_value(def).unwrap_or(serde_json::Value::Null);
    let command = def.get("command").and_then(|v| v.as_str()).map(String::from);
    let url = def
        .get("url")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            // URL 也可能藏在对象形式的 transport 字段里，先只认顶层
            def.get("base_url").and_then(|v| v.as_str()).map(String::from)
        });
    let args = def
        .get("args")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut map = raw.as_object().cloned().unwrap_or_default();
    if url.is_some() && !map.contains_key("command") {
        map.insert("type".into(), serde_json::Value::String("http".into()));
    }
    McpEntry {
        agent_id: agent_id.into(),
        name: name.into(),
        scope: "global".into(),
        transport: detect_transport(&map),
        command,
        args,
        url,
        raw,
    }
}

impl Connector for CodexConnector {
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
        let text = std::fs::read_to_string(&path)?;
        let doc: TomlValue = toml::from_str(&text).map_err(|e| CoreError::TomlParse(e.to_string()))?;
        let empty = toml::map::Map::new();
        let servers = doc
            .get("mcp_servers")
            .and_then(|v| v.as_table())
            .unwrap_or(&empty);
        Ok(servers
            .iter()
            .filter_map(|(name, def)| def.as_table().map(|t| toml_table_to_entry("codex", name, t)))
            .collect())
    }

    fn upsert_mcp(&self, name: &str, def: &McpServerDef) -> Result<WriteReport> {
        let path = self.config_path();
        if !path.exists() {
            return Err(CoreError::NotFound(path.display().to_string()));
        }
        let text = std::fs::read_to_string(&path)?;
        let mut doc: DocumentMut = text
            .parse::<DocumentMut>()
            .map_err(|e| CoreError::TomlParse(e.to_string()))?;

        if doc.get("mcp_servers").is_none() {
            doc.insert("mcp_servers", Item::Table(Table::new()));
        }
        let servers = doc["mcp_servers"]
            .as_table_mut()
            .ok_or_else(|| CoreError::Other("mcp_servers 不是 TOML 表".into()))?;

        let mut t = Table::new();
        if let Some(c) = &def.command {
            t.insert("command", value(c.clone()));
        }
        if !def.args.is_empty() {
            let mut arr = toml_edit::Array::new();
            for a in &def.args {
                arr.push(a.clone());
            }
            t.insert("args", value(arr));
        }
        if let Some(url) = &def.url {
            t.insert("url", value(url.clone()));
        }
        if let Some(env) = &def.env {
            let mut et = Table::new();
            for (k, v) in env {
                match v {
                    serde_json::Value::String(s) => {
                        et.insert(k, value(s.clone()));
                    }
                    serde_json::Value::Bool(b) => {
                        et.insert(k, value(*b));
                    }
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            et.insert(k, value(i));
                        } else if let Some(f) = n.as_f64() {
                            et.insert(k, value(f));
                        }
                    }
                    other => {
                        et.insert(k, value(other.to_string()));
                    }
                }
            }
            t.insert("env", Item::Table(et));
        }
        servers.insert(name, Item::Table(t));

        let backup = snapshot::snapshot_file(&path)?;
        std::fs::write(&path, doc.to_string())?;
        Ok(WriteReport {
            backup_path: Some(backup.display().to_string()),
            changed: true,
        })
    }

    fn remove_mcp(&self, name: &str, scope: &str) -> Result<WriteReport> {
        if scope != "global" {
            return Err(CoreError::Other(format!("未知作用域: {scope}")));
        }
        let path = self.config_path();
        if !path.exists() {
            return Err(CoreError::NotFound(path.display().to_string()));
        }
        let text = std::fs::read_to_string(&path)?;
        let mut doc: DocumentMut = text
            .parse::<DocumentMut>()
            .map_err(|e| CoreError::TomlParse(e.to_string()))?;
        let servers = doc["mcp_servers"]
            .as_table_mut()
            .ok_or_else(|| CoreError::Other("mcp_servers 不是 TOML 表".into()))?;
        if servers.remove(name).is_none() {
            return Err(CoreError::NotFound(name.to_string()));
        }
        let backup = snapshot::snapshot_file(&path)?;
        std::fs::write(&path, doc.to_string())?;
        Ok(WriteReport {
            backup_path: Some(backup.display().to_string()),
            changed: true,
        })
    }
}
