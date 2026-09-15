pub mod claude_code;
pub mod codex;
pub mod cursor;
pub mod dsh;
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

    fn upsert_mcp(&self, _name: &str, _def: &McpServerDef, _scope: &str) -> Result<WriteReport> {
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

/// 极简 YAML frontmatter 解析：只关心 name / description。
/// 支持：单行值、单双引号、行内注释、折叠块（>）、字面块（|）、
/// 空值后的缩进续行（YAML plain 多行标量，真实 SKILL.md 里很常见）、CRLF。
pub fn parse_frontmatter(path: &Path) -> (Option<String>, Option<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_frontmatter_str(&text),
        Err(_) => (None, None),
    }
}

pub fn parse_frontmatter_str(text: &str) -> (Option<String>, Option<String>) {
    let text = text.replace("\r\n", "\n");
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return (None, None);
    }

    #[derive(PartialEq)]
    enum Target {
        Name,
        Desc,
    }
    let mut name: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut target: Option<Target> = None;
    let mut in_block = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let indent = line.len() - line.trim_start().len();

        if in_block {
            if trimmed.is_empty() {
                continue; // 块内空行折叠为空格
            }
            if indent > 0 {
                let piece = trimmed.trim_start_matches("- ").trim();
                let slot = match target {
                    Some(Target::Name) => &mut name,
                    Some(Target::Desc) => &mut desc,
                    None => unreachable!(),
                };
                if let Some(v) = slot {
                    v.push(' ');
                    v.push_str(piece);
                }
                continue;
            }
            in_block = false;
            target = None;
            // 非缩进行继续走下面的键解析
        }

        let Some((key, value)) = split_key(trimmed) else {
            continue;
        };
        let t = match key {
            "name" if name.is_none() => {
                target = Some(Target::Name);
                &mut name
            }
            "description" if desc.is_none() => {
                target = Some(Target::Desc);
                &mut desc
            }
            _ => continue,
        };
        let v = value.trim();
        // `>` / `|` 显式块，或 `key:` 后换行接缩进续行（plain 多行标量），
        // 都走同一条累加路径；后者以前被当成空值直接丢弃，导致描述整段丢失。
        if v.is_empty() || v.starts_with('>') || v.starts_with('|') {
            in_block = true;
            *t = Some(String::new());
        } else {
            *t = Some(clean_yaml_value(v));
            target = None;
        }
    }

    let norm = |s: Option<String>| s.map(|v| collapse_spaces(&v)).filter(|v| !v.is_empty());
    (norm(name), norm(desc))
}

/// 拆 "key: value"（key 不含空格），返回 (key, 其余部分)
fn split_key(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(':')?;
    let key = &line[..idx];
    if key.is_empty() || key.contains(' ') || key.contains('\t') {
        return None;
    }
    Some((key, &line[idx + 1..]))
}

fn clean_yaml_value(v: &str) -> String {
    let v = v.trim();
    // 单引号：'' 是转义的 '
    if let Some(rest) = v.strip_prefix('\'') {
        let mut out = String::new();
        let mut iter = rest.chars().peekable();
        while let Some(c) = iter.next() {
            if c == '\'' {
                if iter.peek() == Some(&'\'') {
                    out.push('\'');
                    iter.next();
                } else {
                    return out; // 结束引号
                }
            } else {
                out.push(c);
            }
        }
        return out;
    }
    // 双引号：取到匹配的结束引号
    if let Some(rest) = v.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return rest[..end].to_string();
        }
        return rest.to_string();
    }
    // 行内注释：` #` 之后截断
    let v = match v.find(" #") {
        Some(pos) => &v[..pos],
        None => v,
    };
    v.trim().to_string()
}

fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// 统计所有已声明 skill 目录下的子目录数。
/// 必须与 scan_skill_dirs 的口径一致（扫全部目录）：只取第一个目录会让
/// 声明了共享目录（如 ~/.agents/skills）的 Agent 徒有偏小的徽标数。
pub fn count_skills(dirs: &OsPaths, base: &Path) -> Option<usize> {
    let mut found_any = false;
    let mut total = 0;
    for dir in resolve_all(dirs, base) {
        if !dir.exists() {
            continue;
        }
        found_any = true;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            total += entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count();
        }
    }
    found_any.then_some(total)
}
