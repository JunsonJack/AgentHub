use agenthub_core::model::{
    AgentStatus, DeployResult, InstallOutcome, MarketPreview, MarketSkill, McpEntry, McpServerDef,
    SkillEntry, SnapshotMeta,
};
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
pub fn list_health_issues(reg: State<Registry>) -> Result<Vec<agenthub_core::model::HealthIssue>, String> {
    Ok(reg.health())
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
pub fn remove_skill(
    reg: State<Registry>,
    agent_id: String,
    skill_name: String,
    scope: String,
) -> Result<agenthub_core::trash::TrashItem, String> {
    reg.remove_skill(&agent_id, &skill_name, &scope)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_trash() -> Result<Vec<agenthub_core::trash::TrashItem>, String> {
    Ok(agenthub_core::trash::list_trash())
}

#[tauri::command]
pub fn restore_trash(id: String) -> Result<String, String> {
    agenthub_core::trash::restore_trash(&id)
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
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

/* ---------- 市场与收藏 ---------- */

const SKILLSMP_KEY_SETTING: &str = "skillsmp_api_key";

#[tauri::command]
pub fn market_search(source: String, q: String, limit: u32) -> Result<Vec<MarketSkill>, String> {
    let key = skillsmp_key();
    match source.as_str() {
        "skillsmp" => agenthub_core::market::search_skillsmp(&q, limit, key.as_deref())
            .map_err(|e| e.to_string()),
        _ => agenthub_core::market::search_skills_sh(&q, limit).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub fn market_preview(id: String) -> Result<MarketPreview, String> {
    agenthub_core::market::preview_skills_sh(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn market_install_skills_sh(
    reg: State<Registry>,
    id: String,
    agent_ids: Vec<String>,
    overwrite: bool,
) -> Result<InstallOutcome, String> {
    let adopt = agenthub_core::market::install_skills_sh(&id, &agenthub_core::util::app_data_dir(), false)
        .map_err(|e| e.to_string())?;
    let deploys = reg.deploy_skill(&adopt.skill_name, &agent_ids, overwrite);
    Ok(InstallOutcome { adopt, deploys })
}

#[tauri::command]
pub fn market_install_git(
    reg: State<Registry>,
    url: String,
    agent_ids: Vec<String>,
    overwrite: bool,
) -> Result<InstallOutcome, String> {
    let adopt = agenthub_core::market::install_git(&url, &agenthub_core::util::app_data_dir(), false)
        .map_err(|e| e.to_string())?;
    let deploys = reg.deploy_skill(&adopt.skill_name, &agent_ids, overwrite);
    Ok(InstallOutcome { adopt, deploys })
}

#[tauri::command]
pub fn deploy_library_skill(
    reg: State<Registry>,
    name: String,
    agent_ids: Vec<String>,
    overwrite: bool,
) -> Result<Vec<DeployResult>, String> {
    Ok(reg.deploy_skill(&name, &agent_ids, overwrite))
}

fn skillsmp_key() -> Option<String> {
    agenthub_core::store::Store::open_default()
        .ok()
        .and_then(|s| s.get_setting(SKILLSMP_KEY_SETTING).ok().flatten())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub set: bool,
    pub masked: Option<String>,
}

#[tauri::command]
pub fn skillsmp_key_status() -> Result<KeyStatus, String> {
    Ok(match skillsmp_key() {
        Some(k) => {
            let tail: String = k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            KeyStatus { set: true, masked: Some(format!("sk_live_****{tail}")) }
        }
        None => KeyStatus { set: false, masked: None },
    })
}

#[tauri::command]
pub fn skillsmp_set_key(key: String) -> Result<(), String> {
    let trimmed = key.trim().to_string();
    if !trimmed.starts_with("sk_") {
        return Err("SkillsMP 密钥通常以 sk_ 开头，请检查后重试".into());
    }
    agenthub_core::store::Store::open_default()
        .map_err(|e| e.to_string())?
        .set_setting(SKILLSMP_KEY_SETTING, &trimmed)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn skillsmp_clear_key() -> Result<(), String> {
    agenthub_core::store::Store::open_default()
        .map_err(|e| e.to_string())?
        .delete_setting(SKILLSMP_KEY_SETTING)
        .map_err(|e| e.to_string())
}
