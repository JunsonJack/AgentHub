pub mod claude_code;
pub mod codex;
pub mod cursor;
pub mod generic;
pub mod json_config;
pub mod zcode;

use std::path::Path;

use crate::error::{CoreError, Result};
use crate::model::{
    AgentDescriptor, AgentStatus, McpEntry, McpServerDef, OsPaths, SkillEntry,
};
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

    /// 默认实现：扫描注册表声明的 skillDirs（user 作用域）；需要项目级的连接器自行覆盖
    fn list_skills(&self) -> Result<Vec<SkillEntry>> {
        Ok(scan_skill_dirs(
            &self.descriptor().skill_dirs,
            self.base_dir(),
            &self.descriptor().id,
        ))
    }

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

/// 下发前的最小 schema 校验：stdio 必须有 command，http/sse 必须有 url
pub fn validate_def(def: &McpServerDef) -> Result<()> {
    let transport = def
        .extra
        .get("type")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            if def.command.is_some() {
                "stdio".into()
            } else if def.url.is_some() {
                "http".into()
            } else {
                "unknown".into()
            }
        });
    match transport.as_str() {
        "stdio" if def.command.as_deref().map_or(true, |c| c.trim().is_empty()) => {
            return Err(CoreError::Other("stdio 传输必须提供 command".into()));
        }
        "http" | "sse" if def.url.as_deref().map_or(true, |u| u.trim().is_empty()) => {
            return Err(CoreError::Other(format!("{transport} 传输必须提供 url")));
        }
        "unknown" => {
            return Err(CoreError::Other(
                "必须提供 command（stdio）或 url（http/sse）之一".into(),
            ));
        }
        _ => {}
    }
    if let Some(env) = &def.env {
        for k in env.keys() {
            if k.trim().is_empty() {
                return Err(CoreError::Other("env 存在空键名".into()));
            }
        }
    }
    Ok(())
}

/// 扫描一个 skill 目录：只认子目录，SKILL.md 的 frontmatter 提供描述
pub fn scan_skill_dir(agent_id: &str, dir: &Path, scope: &str) -> Vec<SkillEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out = vec![];
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let Some(name) = p.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        let skill_md = p.join("SKILL.md");
        let has = skill_md.is_file();
        let description = if has {
            parse_frontmatter(&skill_md).1
        } else {
            None
        };
        out.push(SkillEntry {
            agent_id: agent_id.into(),
            name,
            scope: scope.into(),
            dir: p.display().to_string(),
            description,
            has_skill_md: has,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 扫描注册表声明的全部 skillDirs，统一 user 作用域
pub fn scan_skill_dirs(dirs: &OsPaths, base: &Path, agent_id: &str) -> Vec<SkillEntry> {
    let mut out = vec![];
    for d in resolve_all(dirs, base) {
        if d.is_dir() {
            out.extend(scan_skill_dir(agent_id, &d, "user"));
        }
    }
    out
}

/// 极简 YAML frontmatter 解析：只取 name / description 两个键
pub fn parse_frontmatter(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let mut name = None;
    let mut description = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(v) = trimmed.strip_prefix("name:") {
            name = Some(clean_yaml_value(v));
        } else if let Some(v) = trimmed.strip_prefix("description:") {
            description = Some(clean_yaml_value(v));
        }
    }
    (name, description)
}

fn clean_yaml_value(v: &str) -> String {
    let t = v.trim();
    t.trim_matches('"').trim_matches('\'').to_string()
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
