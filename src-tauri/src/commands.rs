use agenthub_core::model::{AgentStatus, McpEntry};
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
