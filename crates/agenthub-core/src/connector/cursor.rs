use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::model::{AgentDescriptor, McpEntry, McpServerDef};
use crate::registry;

use super::json_config::{
    def_to_json, entries_from_map, obj_at, write_json_preserving,
};
use super::{count_skills, detect_by_paths, Connector, WriteReport};

/// Cursor：`~/.cursor/mcp.json`（官方示例可为 JSONC：带 `//` / `/* */` 注释）。
/// 读：容忍注释；写：结构与键序保留。注释在整体重写时无法原样保留（有快照兜底）。
pub struct CursorConnector {
    desc: AgentDescriptor,
    base: PathBuf,
}

impl CursorConnector {
    pub fn from_home() -> Self {
        Self::new(crate::util::home_base())
    }
    pub fn new(base: PathBuf) -> Self {
        Self::with_descriptor(registry::descriptor("cursor"), base)
    }
    pub fn with_descriptor(desc: AgentDescriptor, base: PathBuf) -> Self {
        Self { desc, base }
    }
    fn config_path(&self) -> PathBuf {
        crate::util::primary_config_path(&self.desc, &self.base, ".cursor/mcp.json")
    }
}

/// 去掉 JSONC 注释，返回 (清理后的文本, 是否去掉过注释)。
/// 字符串内的 `//` 与 `/* */` 不会被误伤。
pub fn strip_jsonc_comments(text: &str) -> (String, bool) {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut in_str = false;
    let mut escaped = false;
    let mut removed = false;

    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        // 字符串外
        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() {
            let n = bytes[i + 1] as char;
            if n == '/' {
                // 行注释：吃到换行（保留换行，避免拼接 token）
                removed = true;
                while i < bytes.len() && bytes[i] as char != '\n' {
                    i += 1;
                }
                continue;
            }
            if n == '*' {
                removed = true;
                i += 2;
                while i + 1 < bytes.len() {
                    if bytes[i] as char == '*' && bytes[i + 1] as char == '/' {
                        i += 2;
                        break;
                    }
                    // 块注释里的换行保留，维持行号大致可读
                    if bytes[i] as char == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    (out, removed)
}

/// 读 JSONC / 严格 JSON
pub fn read_jsonc(path: &Path) -> Result<serde_json::Value> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CoreError::Other(format!("读取 {} 失败: {e}", path.display())))?;
    // 先按严格 JSON 试，失败再剥注释
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => Ok(v),
        Err(_) => {
            let (stripped, _) = strip_jsonc_comments(&text);
            serde_json::from_str(&stripped).map_err(|e| {
                CoreError::Other(format!(
                    "解析 {} 失败（已尝试作为 JSONC 剥离注释）: {e}",
                    path.display()
                ))
            })
        }
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
        let root = read_jsonc(&path)?;
        let empty = serde_json::Map::new();
        let servers = root
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty);
        Ok(entries_from_map("cursor", "global", servers))
    }

    fn upsert_mcp(&self, name: &str, def: &McpServerDef, scope: &str) -> Result<WriteReport> {
        if scope != "global" && !scope.is_empty() {
            return Err(CoreError::Other(format!("未知作用域: {scope}")));
        }
        let path = self.config_path();
        if !path.exists() {
            return Err(CoreError::NotFound(path.display().to_string()));
        }
        let mut root = read_jsonc(&path)?;
        obj_at(&mut root, &["mcpServers"])?.insert(name.into(), def_to_json(def));
        write_json_preserving(&path, &root)
    }

    fn remove_mcp(&self, name: &str, scope: &str) -> Result<WriteReport> {
        if scope != "global" && !scope.is_empty() {
            return Err(CoreError::Other(format!("未知作用域: {scope}")));
        }
        let path = self.config_path();
        if !path.exists() {
            return Err(CoreError::NotFound(path.display().to_string()));
        }
        let mut root = read_jsonc(&path)?;
        if obj_at(&mut root, &["mcpServers"])?.remove(name).is_none() {
            return Err(CoreError::NotFound(format!("{name} @ {scope}")));
        }
        write_json_preserving(&path, &root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_line_comment() {
        let (s, removed) = strip_jsonc_comments("{\n  // hi\n  \"a\": 1\n}\n");
        assert!(removed);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn strip_block_comment() {
        let (s, removed) = strip_jsonc_comments("{ /* x\ny */ \"a\": 1 }");
        assert!(removed);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn does_not_touch_comment_like_in_string() {
        let (s, removed) = strip_jsonc_comments(r#"{ "url": "http://x" }"#);
        assert!(!removed);
        assert!(s.contains("http://x"));
    }
}
