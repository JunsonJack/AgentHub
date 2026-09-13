use std::path::Path;

use serde_json::{Map, Value};

use crate::error::{CoreError, Result};
use crate::model::{McpEntry, McpServerDef};
use crate::snapshot;

use super::WriteReport;

pub fn read_json(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CoreError::Other(format!("读取 {} 失败: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| CoreError::Other(format!("解析 {} 失败: {e}", path.display())))
}

/// 沿 key 链取（或创建）嵌套对象
pub fn obj_at<'a>(root: &'a mut Value, keys: &[&str]) -> Result<&'a mut Map<String, Value>> {
    if !root.is_object() {
        return Err(CoreError::Other("配置根节点不是 JSON 对象".into()));
    }
    let mut cur = root;
    for k in keys {
        if !cur.get(*k).is_some_and(|v| v.is_object()) {
            cur.as_object_mut()
                .expect("checked above")
                .insert(k.to_string(), Value::Object(Map::new()));
        }
        cur = cur.get_mut(*k).expect("just inserted");
    }
    Ok(cur.as_object_mut().expect("is object"))
}

pub fn detect_transport(def: &Map<String, Value>) -> String {
    match def.get("type").and_then(|v| v.as_str()).unwrap_or("") {
        "http" | "streamable-http" => return "http".into(),
        "sse" => return "sse".into(),
        "stdio" => return "stdio".into(),
        _ => {}
    }
    if def.contains_key("command") {
        "stdio".into()
    } else if def.contains_key("url") {
        "http".into()
    } else {
        "unknown".into()
    }
}

pub fn entries_from_map(agent_id: &str, scope: &str, map: &Map<String, Value>) -> Vec<McpEntry> {
    map.iter()
        .map(|(name, def)| {
            let d = def.as_object().cloned().unwrap_or_default();
            McpEntry {
                agent_id: agent_id.into(),
                name: name.clone(),
                scope: scope.into(),
                transport: detect_transport(&d),
                command: d.get("command").and_then(|v| v.as_str()).map(String::from),
                args: d
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                url: d.get("url").and_then(|v| v.as_str()).map(String::from),
                raw: Value::Object(d),
            }
        })
        .collect()
}

/// 严格 JSON 的保留写入：无注释可破坏，serde_json 开了 preserve_order 保键序；
/// 写前自动快照。
pub fn write_json_preserving(path: &Path, root: &Value) -> Result<WriteReport> {
    let backup = snapshot::snapshot_file(path)?;
    let mut out = serde_json::to_string_pretty(root)?;
    out.push('\n');
    std::fs::write(path, out)?;
    Ok(WriteReport {
        backup_path: Some(backup.display().to_string()),
        changed: true,
    })
}

pub fn def_to_json(def: &McpServerDef) -> Value {
    serde_json::to_value(def).unwrap_or(Value::Null)
}
