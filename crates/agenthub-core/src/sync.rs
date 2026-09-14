//! 跨 Agent 同步引擎（P1 第 5 项）：
//! - skill：文件级 diff → 差异预览（dry-run）→ 复制传播；删除差异默认扣留，只有人能确认
//! - mcp：以源 Agent 定义为准传播；env 密钥默认不外带（目标已有则保留本地值，缺失填占位符）
//!
//! 安全语义（借鉴 skills-manager）：
//! - 同步前对目标目录整体快照
//! - "没有任何东西会自行修改"：plan 与 apply 严格分离

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::error::{CoreError, Result};
use crate::model::{DeployResult, McpServerDef};
use crate::registry::Registry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirDiff {
    pub only_in_source: Vec<String>,
    pub only_in_target: Vec<String>,
    pub changed: Vec<String>,
    pub same: usize,
}

/// 递归收集相对路径（统一 / 分隔）
fn collect_rel(root: &Path, out: &mut Vec<String>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
        for e in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out)?;
            } else {
                let rel = p
                    .strip_prefix(root)
                    .map_err(|e| CoreError::Other(e.to_string()))?
                    .to_string_lossy()
                    .to_string();
                out.push(rel.replace('\\', "/"));
            }
        }
        Ok(())
    }
    walk(root, root, out)
}

fn file_bytes(root: &Path, rel: &str) -> Option<Vec<u8>> {
    std::fs::read(root.join(rel)).ok()
}

/// 目录级文件差异（内容逐字节比较；skill 目录都是小文件，无需哈希优化）。
/// 跳过 .git 与 manifest.json（VCS 内部文件 / AgentHub 元数据，都不属于 skill 内容）。
pub fn diff_dirs(source: &Path, target: &Path) -> Result<DirDiff> {
    let mut src = vec![];
    let mut tgt = vec![];
    collect_rel(source, &mut src)?;
    collect_rel(target, &mut tgt)?;
    let is_meta = |f: &String| f == ".git" || f.starts_with(".git/") || f == "manifest.json";
    let src: Vec<String> = src.into_iter().filter(|f| !is_meta(f)).collect();
    let tgt: Vec<String> = tgt.into_iter().filter(|f| !is_meta(f)).collect();
    let mut diff = DirDiff {
        only_in_source: vec![],
        only_in_target: vec![],
        changed: vec![],
        same: 0,
    };
    for f in &src {
        if tgt.contains(f) {
            match (file_bytes(source, f), file_bytes(target, f)) {
                (Some(a), Some(b)) if a == b => diff.same += 1,
                _ => diff.changed.push(f.clone()),
            }
        } else {
            diff.only_in_source.push(f.clone());
        }
    }
    for f in &tgt {
        if !src.contains(f) {
            diff.only_in_target.push(f.clone());
        }
    }
    Ok(diff)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropagatePlan {
    pub target_agent: String,
    /// 目标 skill 目录；target_absent 时为"将创建"的路径
    pub target_dir: String,
    /// 将复制（新增 + 更新）的文件
    pub copy: Vec<String>,
    /// 目标多出的文件——默认扣留，需用户显式确认才会删除
    pub deletions: Vec<String>,
    /// 目标 Agent 上没有该 skill（将整体安装）
    pub target_absent: bool,
    pub identical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub target_agent: String,
    pub copied: usize,
    pub deleted: usize,
    pub held_back_deletions: Vec<String>,
    /// 同步前目标目录的快照位置（可手动恢复）
    pub backup_dir: Option<String>,
}

fn find_entry(
    reg: &crate::registry::Registry,
    agent_id: &str,
    skill_name: &str,
    scope: &str,
) -> Option<crate::model::SkillEntry> {
    reg.all_skills()
        .into_iter()
        .find(|s| s.agent_id == agent_id && s.name == skill_name && s.scope == scope)
}

fn target_skill_root(reg: &crate::registry::Registry, agent_id: &str) -> Result<PathBuf> {
    let c = reg.find_connector(agent_id)?;
    let dirs = crate::util::resolve_all(&c.descriptor().skill_dirs, c.base_dir());
    dirs.first()
        .cloned()
        .ok_or_else(|| CoreError::Other(format!("{agent_id}: 注册表未声明 skill 目录")))
}

/// 生成同步计划（dry-run，只读）
pub fn plan_skill_sync(
    reg: &Registry,
    source_agent: &str,
    skill_name: &str,
    scope: &str,
    target_agent: &str,
) -> Result<PropagatePlan> {
    let source = find_entry(reg, source_agent, skill_name, scope)
        .ok_or_else(|| CoreError::NotFound(format!("{skill_name} @ {source_agent}/{scope}")))?;
    let src_dir = PathBuf::from(&source.dir);

    let target_root = target_skill_root(reg, target_agent)?;
    let target_dir = target_root.join(skill_name);

    let (copy, deletions, identical, target_absent) = plan_dir_sync(&src_dir, &target_dir)?;
    Ok(PropagatePlan {
        target_agent: target_agent.into(),
        target_dir: target_dir.display().to_string(),
        copy,
        deletions,
        target_absent,
        identical,
    })
}

/// 目录对同步原语：比较任意两个目录，返回 (复制清单, 删除清单, 是否一致, 目标缺失)
pub fn plan_dir_sync(source_dir: &Path, target_dir: &Path) -> Result<(Vec<String>, Vec<String>, bool, bool)> {
    if !source_dir.is_dir() {
        return Err(CoreError::NotFound(source_dir.display().to_string()));
    }
    if !target_dir.is_dir() {
        let mut files = vec![];
        collect_rel(source_dir, &mut files)?;
        return Ok((files, vec![], false, true));
    }
    let diff = diff_dirs(source_dir, target_dir)?;
    let identical =
        diff.changed.is_empty() && diff.only_in_source.is_empty() && diff.only_in_target.is_empty();
    Ok((
        [diff.only_in_source, diff.changed].concat(),
        diff.only_in_target,
        identical,
        false,
    ))
}

/// 目标目录整体快照（同步前的安全网）
fn snapshot_target_dir(target_dir: &Path, skill_name: &str) -> Result<String> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = crate::snapshot::snapshots_root()
        .join(format!("{ms}-dir"))
        .join(skill_name);
    crate::library::copy_dir_all(target_dir, &dest)?;
    Ok(dest.display().to_string())
}

/// 执行同步：按 plan 复制；deletions 仅在 delete_confirmed=true 时删除
pub fn apply_skill_sync(
    reg: &Registry,
    source_agent: &str,
    skill_name: &str,
    scope: &str,
    plan: &PropagatePlan,
    delete_confirmed: bool,
) -> Result<SyncReport> {
    let source = find_entry(reg, source_agent, skill_name, scope)
        .ok_or_else(|| CoreError::NotFound(format!("{skill_name} @ {source_agent}/{scope}")))?;
    let src_dir = PathBuf::from(&source.dir);
    let target_dir = PathBuf::from(&plan.target_dir);
    apply_dir_sync(&src_dir, &target_dir, &plan.copy, &plan.deletions, plan.target_absent, delete_confirmed)
}

/// 目录对同步原语：把 copy 列表从源复制到目标；deletions 按确认删除；返回报告
pub fn apply_dir_sync(
    source_dir: &Path,
    target_dir: &Path,
    copy: &[String],
    deletions: &[String],
    target_absent: bool,
    delete_confirmed: bool,
) -> Result<SyncReport> {
    let skill_name = target_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "skill".into());

    let mut report = SyncReport {
        target_agent: String::new(),
        copied: 0,
        deleted: 0,
        held_back_deletions: vec![],
        backup_dir: None,
    };

    if copy.is_empty() && (deletions.is_empty() || !delete_confirmed) {
        if target_absent {
            std::fs::create_dir_all(target_dir)?;
        }
        report.held_back_deletions = deletions.to_vec();
        return Ok(report);
    }

    if target_dir.is_dir() {
        report.backup_dir = Some(snapshot_target_dir(target_dir, &skill_name)?);
    } else {
        std::fs::create_dir_all(target_dir)?;
    }

    for rel in copy {
        let src = source_dir.join(rel);
        let dest = target_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dest)?;
        report.copied += 1;
    }

    if delete_confirmed {
        for rel in deletions {
            let p = target_dir.join(rel);
            if p.is_file() {
                std::fs::remove_file(&p)?;
                report.deleted += 1;
            }
        }
    } else {
        report.held_back_deletions = deletions.to_vec();
    }
    Ok(report)
}

/* ---------- MCP 传播（密钥不外带） ---------- */

/// 常见密钥命名约定（TOKEN/KEY/SECRET/PASSWORD/CREDENTIAL 等）
fn is_secret_key(k: &str) -> bool {
    let u = k.to_uppercase();
    ["TOKEN", "KEY", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL", "APIKEY"].iter().any(|s| u.contains(s))
}

/// env 合并策略：密钥类键——目标已有则保留本地值，否则占位符；
/// 非密钥键照常从源同步。（同步引擎与 Profile 应用共用）
pub(crate) fn merge_env_for_target(
    source_env: &Map<String, serde_json::Value>,
    target_env: Option<&Map<String, serde_json::Value>>,
) -> Map<String, serde_json::Value> {
    let mut out = Map::new();
    for (k, v) in source_env {
        if is_secret_key(k) {
            match target_env.and_then(|t| t.get(k)) {
                Some(existing) => {
                    out.insert(k.clone(), existing.clone());
                }
                None => {
                    out.insert(
                        k.clone(),
                        serde_json::Value::String("<请在目标 Agent 中填写>".into()),
                    );
                }
            }
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// 以源 Agent 的定义为准，把某个 MCP server 传播到目标 Agent。
/// env 策略（规划红线：密钥默认不跨 Agent 同步）：目标已有同名条目 → env 键对齐但值保留目标本地；
/// 目标没有 → env 值填占位符，由用户在目标侧填写。
pub fn propagate_mcp(
    reg: &crate::registry::Registry,
    source_agent: &str,
    name: &str,
    target_agents: &[String],
) -> Vec<DeployResult> {
    let all = reg.all_mcp();
    let Some(source_entry) = all
        .iter()
        .find(|e| e.agent_id == source_agent && e.name == name && e.scope == "global")
    else {
        return target_agents
            .iter()
            .map(|t| DeployResult {
                agent_id: t.clone(),
                ok: false,
                error: Some(format!("源条目不存在: {name} @ {source_agent}")),
                backup_path: None,
            })
            .collect();
    };
    let source_env = source_entry
        .raw
        .get("env")
        .and_then(|v| v.as_object())
        .cloned();
    let mut def: McpServerDef =
        serde_json::from_value(source_entry.raw.clone()).unwrap_or_default();

    let mut out = vec![];
    for target in target_agents {
        let target_env = all
            .iter()
            .find(|e| e.agent_id == *target && e.name == name && e.scope == "global")
            .and_then(|e| e.raw.get("env").and_then(|v| v.as_object()).cloned());
        if let Some(src_env) = &source_env {
            def.env = Some(merge_env_for_target(src_env, target_env.as_ref()));
        }
        out.extend(reg.deploy_mcp(std::slice::from_ref(target), name, &def));
    }
    out
}
