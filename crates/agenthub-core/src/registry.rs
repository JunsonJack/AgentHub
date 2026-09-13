use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::connector::{self, Connector};
use crate::error::Result;
use crate::model::{AgentDescriptor, AgentStatus, McpEntry};

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
}
