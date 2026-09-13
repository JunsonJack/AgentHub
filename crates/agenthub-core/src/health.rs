//! 配置健康度：解析失败、非法 MCP 条目、命令不可达、skill 缺 SKILL.md。

use std::path::{Path, PathBuf};

use crate::connector::Connector;
use crate::model::{HealthIssue, McpEntry};

pub fn check_connector(c: &dyn Connector) -> Vec<HealthIssue> {
    let id = c.descriptor().id.clone();
    let mut out = vec![];

    match c.list_mcp() {
        Err(e) => out.push(HealthIssue {
            agent_id: id.clone(),
            severity: "error".into(),
            code: "CONFIG_PARSE_ERROR".into(),
            message: format!("MCP 配置解析失败：{e}"),
            path: None,
        }),
        Ok(entries) => {
            for e in &entries {
                out.extend(check_entry(&id, e));
            }
        }
    }

    for s in c.list_skills().unwrap_or_default() {
        if !s.has_skill_md {
            out.push(HealthIssue {
                agent_id: id.clone(),
                severity: "warning".into(),
                code: "SKILL_NO_SKILLMD".into(),
                message: format!("skill 「{}」缺少 SKILL.md，Agent 大概率不会加载它", s.name),
                path: Some(s.dir),
            });
        }
    }
    out
}

fn check_entry(agent_id: &str, e: &McpEntry) -> Vec<HealthIssue> {
    let mut out = vec![];
    if e.name.trim().is_empty() {
        out.push(HealthIssue {
            agent_id: agent_id.into(),
            severity: "error".into(),
            code: "MCP_ENTRY_INVALID".into(),
            message: "存在名称为空的 MCP 条目".into(),
            path: None,
        });
        return out;
    }
    match e.transport.as_str() {
        "stdio" => {
            let cmd = e.command.clone().unwrap_or_default();
            if cmd.trim().is_empty() {
                out.push(HealthIssue {
                    agent_id: agent_id.into(),
                    severity: "error".into(),
                    code: "MCP_ENTRY_INVALID".into(),
                    message: format!("stdio 条目「{}」缺少 command", e.name),
                    path: None,
                });
            } else if !find_on_path(&cmd) {
                out.push(HealthIssue {
                    agent_id: agent_id.into(),
                    severity: "warning".into(),
                    code: "MCP_CMD_NOT_FOUND".into(),
                    message: format!(
                        "条目「{}」的启动命令 {} 在 PATH 中找不到（可能已卸载或需要 npx/uvx 拉取）",
                        e.name, cmd
                    ),
                    path: None,
                });
            }
        }
        "http" | "sse" => {
            if e.url.as_deref().map_or(true, |u| u.trim().is_empty()) {
                out.push(HealthIssue {
                    agent_id: agent_id.into(),
                    severity: "error".into(),
                    code: "MCP_ENTRY_INVALID".into(),
                    message: format!("{} 条目「{}」缺少 url", e.transport, e.name),
                    path: None,
                });
            }
        }
        _ => out.push(HealthIssue {
            agent_id: agent_id.into(),
            severity: "error".into(),
            code: "MCP_ENTRY_INVALID".into(),
            message: format!(
                "条目「{}」既没有 command 也没有 url，无法判断传输方式",
                e.name
            ),
            path: None,
        }),
    }
    out
}

/// 判断启动命令在 PATH 上是否可达（取首个 token；Windows 下补查 .exe/.cmd/.bat）
pub fn find_on_path(cmd: &str) -> bool {
    let first = cmd.split_whitespace().next().unwrap_or("").trim_matches('"');
    if first.is_empty() {
        return false;
    }
    let p = Path::new(first);
    if p.components().count() > 1 {
        // 带目录部分：直接按路径判定
        return p.is_file()
            || PathBuf::from(format!("{first}.exe")).is_file();
    }
    let path_var = match std::env::var_os("PATH") {
        Some(v) => v,
        None => return false,
    };
    for dir in std::env::split_paths(&path_var) {
        for ext in ["", ".exe", ".cmd", ".bat"] {
            if dir.join(format!("{first}{ext}")).is_file() {
                return true;
            }
        }
    }
    false
}
