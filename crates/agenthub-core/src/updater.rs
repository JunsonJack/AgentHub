//! Skill 版本与更新（P2）：check 与 apply 严格分离——没有任何东西会自行下载或安装。
//! 支持来源：`git:<url>`（浅克隆）与 `skills.sh:<owner/repo/slug>`（快照）。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::library;
use crate::market;
use crate::registry::Registry;
use crate::sync;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub name: String,
    pub source: String,
    /// 该条目是否有可检查的上游来源
    pub checkable: bool,
    pub has_updates: bool,
    pub identical: bool,
    /// 上游新增的文件
    pub incoming: Vec<String>,
    /// 上游有变更的文件（更新将覆盖）
    pub changed: Vec<String>,
    /// 上游已删除的文件（更新将移除，需用户确认）
    pub upstream_removed: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApplyReport {
    pub name: String,
    /// 中央库已更新
    pub updated_library: bool,
    pub file_count: usize,
    /// 同步到已部署 Agent 的报告（sync_agents=true 时）
    pub synced: Vec<sync::SyncReport>,
    pub error: Option<String>,
}

/// 解析条目的上游引用：source_agent 里的 `git:` / `skills.sh:` 前缀优先
fn upstream_ref(item: &crate::model::LibraryItem) -> Option<String> {
    let agent = item.source_agent.as_deref()?;
    if agent.starts_with("git:") || agent.starts_with("skills.sh:") {
        return Some(agent.to_string());
    }
    let path = item.source_path.as_deref()?;
    if path.starts_with("git:") || path.starts_with("skills.sh:") {
        return Some(path.to_string());
    }
    None
}

/// 检查单个中央库条目的上游更新（只读，不写任何东西）
pub fn check_update(data_root: &Path, name: &str) -> Result<UpdateCheck> {
    let item = library::read_manifest(data_root, name)?;
    let source = upstream_ref(&item).unwrap_or_default();

    let staged = match stage_latest(&source, name) {
        Ok(s) => s,
        Err(e) => {
            return Ok(UpdateCheck {
                name: name.into(),
                source,
                checkable: false,
                has_updates: false,
                identical: false,
                incoming: vec![],
                changed: vec![],
                upstream_removed: vec![],
                error: Some(e.to_string()),
            });
        }
    };
    let result = (|| {
        let (incoming, changed, upstream_removed, identical) =
            diff_against_library(data_root, name, &staged)?;
        let has_updates = !identical;
        Ok(UpdateCheck {
            name: name.into(),
            source,
            checkable: true,
            has_updates,
            identical,
            incoming,
            changed,
            upstream_removed,
            error: None,
        })
    })();
    let _ = std::fs::remove_dir_all(&staged.cleanup_root);
    result
}

/// 应用更新：覆盖中央库条目（先整体快照），可选地把变更同步到已部署该 skill 的 Agent
pub fn apply_update(
    data_root: &Path,
    name: &str,
    reg: &Registry,
    sync_agents: bool,
    delete_confirmed: bool,
) -> Result<UpdateApplyReport> {
    let check = check_update(data_root, name)?;
    if !check.checkable {
        return Err(CoreError::Other(
            check.error.unwrap_or_else(|| "无上游来源".into()),
        ));
    }
    if check.identical {
        return Ok(UpdateApplyReport {
            name: name.into(),
            updated_library: false,
            file_count: library::count_files(&library::skills_root(data_root).join(name))?,
            synced: vec![],
            error: None,
        });
    }

    let staged = stage_latest(&check.source, name)?;
    let result = (|| {
        let lib_dir = library::skills_root(data_root).join(name);
        // 删除前先把清单读出来（上游内容不含 manifest.json，拷完要写回）
        let mut item = library::read_manifest(data_root, name)?;

        // 1) 中央库目录整体快照（可手动恢复）
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let backup = crate::snapshot::snapshots_root()
            .join(format!("{ms}-dir"))
            .join(name);
        library::copy_dir_all(&lib_dir, &backup)?;

        // 2) 用上游内容整体替换中央库目录
        std::fs::remove_dir_all(&lib_dir)?;
        library::copy_dir_all(&staged.content_dir, &lib_dir)?;
        item.file_count = library::count_files(&lib_dir)?;
        item.updated_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        );
        library::write_manifest_item(data_root, name, &item)?;

        let mut report = UpdateApplyReport {
            name: name.into(),
            updated_library: true,
            file_count: item.file_count,
            synced: vec![],
            error: None,
        };

        // 3) 可选：同步到已部署该 skill 的 Agent（源 = 更新后的中央库目录）
        if sync_agents {
            let targets: Vec<String> = reg
                .all_skills()
                .into_iter()
                .filter(|s| s.name == name && s.agent_id != "__library__")
                .map(|s| s.agent_id)
                .collect();
            let mut seen = std::collections::HashSet::new();
            for t in targets {
                if !seen.insert(t.clone()) {
                    continue;
                }
                let target_dir = agent_skill_dir(reg, &t, name)?;
                let (copy, deletions, identical, absent) =
                    sync::plan_dir_sync(&lib_dir, &target_dir)?;
                if absent {
                    // 目标没装过，不主动安装——更新只影响已部署的
                    continue;
                }
                if identical {
                    continue;
                }
                let mut r = sync::apply_dir_sync(
                    &lib_dir,
                    &target_dir,
                    &copy,
                    &deletions,
                    false,
                    delete_confirmed,
                )?;
                r.target_agent = t.clone();
                report.synced.push(r);
            }
        }
        Ok(report)
    })();
    let _ = std::fs::remove_dir_all(&staged.cleanup_root);
    result
}

fn agent_skill_dir(reg: &Registry, agent_id: &str, skill_name: &str) -> Result<std::path::PathBuf> {
    let c = reg.find_connector(agent_id)?;
    let dirs = crate::util::resolve_all(&c.descriptor().skill_dirs, c.base_dir());
    Ok(dirs
        .first()
        .cloned()
        .unwrap_or_else(|| std::path::PathBuf::from(".").join(skill_name))
        .join(skill_name))
}

/// 按来源拉取最新版本到临时目录
fn stage_latest(source: &str, _name: &str) -> Result<market::FetchedUpstream> {
    if let Some(url) = source.strip_prefix("git:") {
        market::fetch_git_to_temp(url)
    } else if let Some(id) = source.strip_prefix("skills.sh:") {
        market::fetch_skills_sh_to_temp(id)
    } else {
        Err(CoreError::Other(format!(
            "该条目没有可检查的上游来源（{source}）"
        )))
    }
}

/// 上游目录 vs 中央库目录的差异（manifest.json 是 AgentHub 元数据，不参与比较）
fn diff_against_library(
    data_root: &Path,
    name: &str,
    staged: &market::FetchedUpstream,
) -> Result<(Vec<String>, Vec<String>, Vec<String>, bool)> {
    let lib_dir = library::skills_root(data_root).join(name);
    if !lib_dir.is_dir() {
        return Err(CoreError::NotFound(lib_dir.display().to_string()));
    }
    let diff = sync::diff_dirs(&staged.content_dir, &lib_dir)?;
    let upstream_removed: Vec<String> = diff
        .only_in_target
        .into_iter()
        .filter(|f| f != "manifest.json")
        .collect();
    let identical = diff.changed.is_empty() && diff.only_in_source.is_empty() && upstream_removed.is_empty();
    Ok((diff.only_in_source, diff.changed, upstream_removed, identical))
}
