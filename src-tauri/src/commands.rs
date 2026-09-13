use agenthub_core::model::{AgentStatus, DeployResult, McpEntry, McpServerDef, SkillEntry, SnapshotMeta};
use agenthub_core::registry::Registry;
use tauri::State;

#[tauri::command]
pub fn list_agents(reg: State<Registry>) -> Result<Vec<AgentStatus>, String> {
    Ok(reg.statuses())
}

#[tauri::command]
pub fn list_mcp_all(reg: State<Registry>) -> Result<Vec<McpEntry>, String> {
    Ok(reg.all_mcp())
}

#[tauri::command]
pub fn deploy_mcp(
    reg: State<Registry>,
    agent_ids: Vec<String>,
    name: String,
    def: McpServerDef,
) -> Result<Vec<DeployResult>, String> {
    Ok(reg.deploy_mcp(&agent_ids, &name, &def))
}

#[tauri::command]
pub fn remove_mcp(
    reg: State<Registry>,
    agent_id: String,
    name: String,
    scope: String,
) -> Result<agenthub_core::connector::WriteReport, String> {
    reg.remove_mcp_for(&agent_id, &name, &scope).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_skills(reg: State<Registry>) -> Result<Vec<SkillEntry>, String> {
    Ok(reg.all_skills())
}

#[tauri::command]
pub fn adopt_skill(
    reg: State<Registry>,
    agent_id: String,
    skill_name: String,
    scope: String,
    dry_run: bool,
) -> Result<agenthub_core::model::AdoptReport, String> {
    reg.adopt_skill(&agent_id, &skill_name, &scope, dry_run)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_library() -> Result<Vec<agenthub_core::model::LibraryItem>, String> {
    Ok(agenthub_core::library::list_library())
}

#[tauri::command]
pub fn read_library_skill(name: String) -> Result<String, String> {
    agenthub_core::library::read_library_skill_md(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_snapshots() -> Result<Vec<SnapshotMeta>, String> {
    Ok(agenthub_core::snapshot::list_snapshots())
}

#[tauri::command]
pub fn rollback_snapshot(id: String, file_name: String) -> Result<String, String> {
    agenthub_core::snapshot::rollback(&id, &file_name)
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

/// 读取任意 skill 目录下的 SKILL.md（详情抽屉用；本地单机软件，路径来自 list_skills）
#[tauri::command]
pub fn read_skill_md(dir: String) -> Result<String, String> {
    let path = std::path::Path::new(&dir).join("SKILL.md");
    if !path.is_file() {
        return Ok(String::new());
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}
