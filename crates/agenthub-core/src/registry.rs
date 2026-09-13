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

pub struct Registry {
    connectors: Vec<Box<dyn Connector>>,
}

impl Registry {
    pub fn load() -> Result<Self> {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Ok(Self::with_base(base))
    }

    pub fn with_base(base: PathBuf) -> Self {
        Self {
            connectors: descriptors()
                .iter()
                .map(|d| connector::generic::make_connector(&d.id, base.clone()))
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

    fn find(&self, agent_id: &str) -> Result<&dyn Connector> {
        self.connectors
            .iter()
            .find(|c| c.descriptor().id == agent_id)
            .map(|c| c.as_ref())
            .ok_or_else(|| CoreError::NotFound(format!("agent {agent_id}")))
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
}
