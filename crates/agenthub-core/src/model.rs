use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentKind {
    Cli,
    Ide,
    Desktop,
}

/// 注册表中按操作系统声明的候选路径
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OsPaths {
    #[serde(default)]
    pub windows: Vec<String>,
    #[serde(default)]
    pub macos: Vec<String>,
    #[serde(default)]
    pub linux: Vec<String>,
}

/// Agent 注册表条目：路径与格式声明是数据而非硬编码（借鉴 skills-manager）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDescriptor {
    pub id: String,
    pub name: String,
    pub kind: AgentKind,
    pub mcp_config_paths: OsPaths,
    #[serde(default)]
    pub skill_dirs: OsPaths,
    #[serde(default = "default_reload")]
    pub reload: String,
}

fn default_reload() -> String {
    "restart".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub id: String,
    pub name: String,
    pub kind: AgentKind,
    pub installed: bool,
    pub found_paths: Vec<String>,
    pub mcp_count: Option<usize>,
    pub skill_count: Option<usize>,
    pub health_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpEntry {
    pub agent_id: String,
    pub name: String,
    /// "global" 或 "project:<路径>"
    pub scope: String,
    /// stdio | http | sse | unknown
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    /// 原始定义（展示 / 编辑用，含 env 等全部字段）
    pub raw: serde_json::Value,
}

/// 写入时的受控定义：经表单编辑后下发用
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    pub agent_id: String,
    /// 目录名（skill 的稳定标识）
    pub name: String,
    /// "user" 或 "project:<路径>"
    pub scope: String,
    /// 绝对路径
    pub dir: String,
    pub description: Option<String>,
    pub has_skill_md: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub name: String,
    /// "skill"
    pub kind: String,
    pub source_agent: Option<String>,
    pub source_path: Option<String>,
    pub description: Option<String>,
    pub adopted_at: u64,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptReport {
    pub dry_run: bool,
    pub skill_name: String,
    pub target_dir: String,
    /// 相对文件路径清单（dry-run 即"将要复制什么"）
    pub files: Vec<String>,
    /// 目标已存在（拒绝覆盖，遵循"绝不代删"语义）
    pub conflict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMeta {
    /// 快照目录名（unix 毫秒）
    pub id: String,
    pub file_name: String,
    pub original_path: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployResult {
    pub agent_id: String,
    pub ok: bool,
    pub error: Option<String>,
    pub backup_path: Option<String>,
}
