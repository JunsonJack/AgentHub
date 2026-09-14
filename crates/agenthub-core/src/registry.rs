use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::connector::{self, Connector};
use crate::error::{CoreError, Result};
use crate::model::{
    AgentDescriptor, AgentStatus, DeployResult, McpEntry, McpServerDef, SkillEntry,
};

pub const REGISTRY_JSON: &str = include_str!("registry.json");

#[derive(Deserialize)]
struct RegistryFile {
    #[allow(dead_code)]
    version: u32,
    agents: Vec<AgentDescriptor>,
}

static DESCRIPTORS: OnceLock<Vec<AgentDescriptor>> = OnceLock::new();

pub fn descriptors() -> &'static [AgentDescriptor] {
    DESCRIPTORS.get_or_init(|| {
        let file: RegistryFile = serde_json::from_str(REGISTRY_JSON).expect("registry.json 合法");
        file.agents
    })
}

pub fn descriptor(id: &str) -> AgentDescriptor {
    descriptors()
        .iter()
        .find(|d| d.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("registry.json 中不存在 id={id} 的 Agent"))
}

/// 把用户覆写应用到当前 OS 的路径声明上
fn apply_overrides(descs: &mut [AgentDescriptor], overrides: &serde_json::Value) {
    let Some(map) = overrides.as_object() else { return };
    for d in descs.iter_mut() {
        let Some(spec) = map.get(&d.id).and_then(|v| v.as_object()) else {
            continue;
        };
        if let Some(list) = string_list(spec.get("mcpConfigPaths")) {
            set_current_os(&mut d.mcp_config_paths, list);
        }
        if let Some(list) = string_list(spec.get("skillDirs")) {
            set_current_os(&mut d.skill_dirs, list);
        }
    }
}

fn string_list(v: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let arr = v?.as_array()?;
    let list: Vec<String> = arr
        .iter()
        .filter_map(|p| p.as_str().map(String::from))
        .filter(|s| !s.trim().is_empty())
        .collect();
    Some(list)
}

fn set_current_os(paths: &mut crate::model::OsPaths, list: Vec<String>) {
    if cfg!(target_os = "windows") {
        paths.windows = list;
    } else if cfg!(target_os = "macos") {
        paths.macos = list;
    } else {
        paths.linux = list;
    }
}

pub struct Registry {
    connectors: Vec<Box<dyn Connector>>,
}

/// 设置键：路径覆写 JSON {"agent-id": {"mcpConfigPaths": [...], "skillDirs": [...]}}
pub const OVERRIDES_KEY: &str = "path_overrides";
/// 用户自定义 Agent 描述数组（统一使用标准 mcpServers JSON 格式）。
pub const CUSTOM_AGENTS_KEY: &str = "custom_agents";

impl Registry {
    pub fn load() -> Result<Self> {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        // 覆写读取失败不阻断启动（退化为默认路径）
        let overrides = crate::store::Store::open_default()
            .ok()
            .and_then(|s| s.get_setting(OVERRIDES_KEY).ok().flatten())
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .unwrap_or(serde_json::Value::Null);
        let custom = crate::store::Store::open_default()
            .ok()
            .and_then(|s| s.get_setting(CUSTOM_AGENTS_KEY).ok().flatten())
            .and_then(|raw| serde_json::from_str::<Vec<AgentDescriptor>>(&raw).ok())
            .unwrap_or_default();
        Ok(Self::with_overrides_and_custom(base, &overrides, custom))
    }

    pub fn with_base(base: PathBuf) -> Self {
        Self::with_overrides(base, &serde_json::Value::Null)
    }

    pub fn with_overrides(base: PathBuf, overrides: &serde_json::Value) -> Self {
        Self::with_overrides_and_custom(base, overrides, vec![])
    }

    pub fn with_overrides_and_custom(
        base: PathBuf,
        overrides: &serde_json::Value,
        custom: Vec<AgentDescriptor>,
    ) -> Self {
        let mut descs: Vec<AgentDescriptor> = descriptors().iter().cloned().collect();
        let built_in: std::collections::HashSet<String> = descs.iter().map(|d| d.id.clone()).collect();
        descs.extend(custom.into_iter().filter(|d| !built_in.contains(&d.id)));
        apply_overrides(&mut descs, overrides);
        Self {
            connectors: descs
                .iter()
                .map(|d| connector::generic::make_connector(d, base.clone()))
                .collect(),
        }
    }

    /// 每个 Agent 的探测状态；单连接器出错不影响其他 Agent
    pub fn statuses(&self) -> Vec<AgentStatus> {
        self.connectors
            .iter()
            .map(|c| connector::generic::status_of(c.as_ref()))
            .collect()
    }

    /// 跨 Agent 汇总的 MCP 条目（含各作用域）
    pub fn all_mcp(&self) -> Vec<McpEntry> {
        self.connectors
            .iter()
            .flat_map(|c| c.list_mcp().unwrap_or_default())
            .collect()
    }

    /// 跨 Agent 汇总的 Skill 条目
    pub fn all_skills(&self) -> Vec<SkillEntry> {
        self.connectors
            .iter()
            .flat_map(|c| c.list_skills().unwrap_or_default())
            .collect()
    }

    /// 全量健康度检查（解析错误 / 非法条目 / 命令不可达 / skill 缺 SKILL.md）
    pub fn health(&self) -> Vec<crate::model::HealthIssue> {
        self.connectors
            .iter()
            .flat_map(|c| crate::health::check_connector(c.as_ref()))
            .collect()
    }

    fn find(&self, agent_id: &str) -> Result<&dyn Connector> {
        self.connectors
            .iter()
            .find(|c| c.descriptor().id == agent_id)
            .map(|c| c.as_ref())
            .ok_or_else(|| CoreError::NotFound(format!("agent {agent_id}")))
    }

    /// 同步引擎等需要按 id 取连接器（路径声明等）
    pub fn find_connector(&self, agent_id: &str) -> Result<&dyn Connector> {
        self.find(agent_id)
    }

    /// 把一个 MCP server 定义批量下发到多个 Agent（写入前各连接器自动快照）
    pub fn deploy_mcp(
        &self,
        agent_ids: &[String],
        name: &str,
        def: &McpServerDef,
    ) -> Vec<DeployResult> {
        if let Err(e) = connector::validate_def(def) {
            return agent_ids
                .iter()
                .map(|id| DeployResult {
                    agent_id: id.clone(),
                    ok: false,
                    error: Some(e.to_string()),
                    backup_path: None,
                })
                .collect();
        }
        agent_ids
            .iter()
            .map(|id| match self.find(id).and_then(|c| c.upsert_mcp(name, def)) {
                Ok(r) => DeployResult {
                    agent_id: id.clone(),
                    ok: true,
                    error: None,
                    backup_path: r.backup_path,
                },
                Err(e) => DeployResult {
                    agent_id: id.clone(),
                    ok: false,
                    error: Some(e.to_string()),
                    backup_path: None,
                },
            })
            .collect()
    }

    pub fn remove_mcp_for(&self, agent_id: &str, name: &str, scope: &str) -> Result<connector::WriteReport> {
        self.find(agent_id)?.remove_mcp(name, scope)
    }

    /// 收编：按 (agent, skill 名, 作用域) 定位目录后拷入中央库
    pub fn adopt_skill(
        &self,
        agent_id: &str,
        skill_name: &str,
        scope: &str,
        dry_run: bool,
    ) -> Result<crate::model::AdoptReport> {
        let entry = self
            .all_skills()
            .into_iter()
            .find(|s| s.agent_id == agent_id && s.name == skill_name && s.scope == scope)
            .ok_or_else(|| CoreError::NotFound(format!("{skill_name} @ {agent_id}/{scope}")))?;
        crate::library::adopt(std::path::Path::new(&entry.dir), agent_id, dry_run)
    }

    /// 删除 Agent 里的 skill：先移入回收站（可恢复），绝不直接销毁
    pub fn remove_skill(
        &self,
        agent_id: &str,
        skill_name: &str,
        scope: &str,
    ) -> Result<crate::trash::TrashItem> {
        self.remove_skill_in(&crate::trash::trash_root(), agent_id, skill_name, scope)
    }

    /// 同上，回收站位置可注入（测试用）
    pub fn remove_skill_in(
        &self,
        trash_root: &std::path::Path,
        agent_id: &str,
        skill_name: &str,
        scope: &str,
    ) -> Result<crate::trash::TrashItem> {
        let entry = self
            .all_skills()
            .into_iter()
            .find(|s| s.agent_id == agent_id && s.name == skill_name && s.scope == scope)
            .ok_or_else(|| CoreError::NotFound(format!("{skill_name} @ {agent_id}/{scope}")))?;
        crate::trash::trash_dir_in(trash_root, std::path::Path::new(&entry.dir), agent_id)
    }

    /// 禁用 MCP：移出配置文件（走快照）+ 完整定义记入本地禁用清单
    pub fn disable_mcp(
        &self,
        store: &crate::store::Store,
        agent_id: &str,
        name: &str,
        scope: &str,
    ) -> Result<connector::WriteReport> {
        let c = self.find(agent_id)?;
        let entry = c
            .list_mcp()?
            .into_iter()
            .find(|e| e.name == name && e.scope == scope)
            .ok_or_else(|| CoreError::NotFound(format!("{name} @ {agent_id}/{scope}")))?;
        let report = c.remove_mcp(name, scope)?;
        store.add_disabled_mcp(agent_id, name, scope, &entry.raw)?;
        Ok(report)
    }

    /// 启用：从禁用清单取出记录，把原始定义写回 Agent（同名条目会被覆盖）
    pub fn enable_mcp(
        &self,
        store: &crate::store::Store,
        record_id: i64,
    ) -> Result<connector::WriteReport> {
        let record = store
            .take_disabled_mcp(record_id)?
            .ok_or_else(|| CoreError::NotFound(format!("禁用记录 {record_id}")))?;
        let def: McpServerDef = serde_json::from_value(record.def)?;
        self.find(&record.agent_id)?.upsert_mcp(&record.name, &def)
    }

    /// 把中央库里的 skill 复制安装到多个 Agent 的用户级 skill 目录。
    /// 目标已存在且 !overwrite 时拒绝（绝不代删）。
    pub fn deploy_skill(
        &self,
        name: &str,
        agent_ids: &[String],
        overwrite: bool,
    ) -> Vec<DeployResult> {
        self.deploy_skill_in(&crate::util::app_data_dir(), name, agent_ids, overwrite)
    }

    /// 同上，但中央库位置可注入（测试用临时目录）
    pub fn deploy_skill_in(
        &self,
        data_root: &std::path::Path,
        name: &str,
        agent_ids: &[String],
        overwrite: bool,
    ) -> Vec<DeployResult> {
        let src = crate::library::skills_root(data_root).join(name);
        if !src.is_dir() {
            return agent_ids
                .iter()
                .map(|id| DeployResult {
                    agent_id: id.clone(),
                    ok: false,
                    error: Some(format!("中央库不存在 skill: {name}")),
                    backup_path: None,
                })
                .collect();
        }
        agent_ids
            .iter()
            .map(|id| {
                let r = (|| {
                    let c = self.find(id)?;
                    let dirs = crate::util::resolve_all(&c.descriptor().skill_dirs, c.base_dir());
                    let root = dirs
                        .first()
                        .ok_or_else(|| CoreError::Other(format!("{id}: 注册表未声明 skill 目录")))?;
                    std::fs::create_dir_all(root)?;
                    let target = root.join(name);
                    if target.exists() && !overwrite {
                        return Err(CoreError::Other(format!(
                            "目标已存在，拒绝覆盖（如需覆盖请显式选择）: {}",
                            target.display()
                        )));
                    }
                    crate::library::copy_dir_all(&src, &target)?;
                    Ok(target.display().to_string())
                })();
                match r {
                    Ok(_) => DeployResult {
                        agent_id: id.clone(),
                        ok: true,
                        error: None,
                        backup_path: None,
                    },
                    Err(e) => DeployResult {
                        agent_id: id.clone(),
                        ok: false,
                        error: Some(e.to_string()),
                        backup_path: None,
                    },
                }
            })
            .collect()
    }
}
