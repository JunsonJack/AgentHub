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
pub fn disable_mcp(
    reg: State<Registry>,
    agent_id: String,
    name: String,
    scope: String,
) -> Result<agenthub_core::connector::WriteReport, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    reg.disable_mcp(&store, &agent_id, &name, &scope)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_disabled_mcp() -> Result<Vec<agenthub_core::store::DisabledRecord>, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    store.list_disabled_mcp().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn enable_mcp(reg: State<Registry>, record_id: i64) -> Result<agenthub_core::connector::WriteReport, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    reg.enable_mcp(&store, record_id).map_err(|e| e.to_string())
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

/// 从本地文件夹导入 skill 到中央库（dry-run 先出预览）
#[tauri::command]
pub fn import_skill_folder(path: String, dry_run: bool) -> Result<agenthub_core::model::AdoptReport, String> {
    agenthub_core::library::adopt(
        std::path::Path::new(&path),
        "manual",
        dry_run,
    )
    .map_err(|e| e.to_string())
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

#[tauri::command]
pub fn prune_snapshots(keep: u32) -> Result<usize, String> {
    agenthub_core::snapshot::prune(keep as usize).map_err(|e| e.to_string())
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

/* ---------- MCP 连通性测试 ---------- */

#[tauri::command]
pub fn test_mcp(
    reg: State<Registry>,
    agent_id: String,
    name: String,
    scope: String,
) -> Result<agenthub_core::runner::ConnectivityResult, String> {
    Ok(agenthub_core::runner::test_entry(&reg, &agent_id, &name, &scope))
}

#[tauri::command]
pub fn test_mcp_def(
    def: McpServerDef,
) -> Result<agenthub_core::runner::ConnectivityResult, String> {
    Ok(agenthub_core::runner::test_def(&def))
}

/* ---------- 跨 Agent 同步 ---------- */

#[tauri::command]
pub fn plan_skill_sync(
    reg: State<Registry>,
    source_agent: String,
    skill_name: String,
    scope: String,
    target_agent: String,
) -> Result<agenthub_core::sync::PropagatePlan, String> {
    agenthub_core::sync::plan_skill_sync(&reg, &source_agent, &skill_name, &scope, &target_agent)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_skill_sync(
    reg: State<Registry>,
    source_agent: String,
    skill_name: String,
    scope: String,
    plan: agenthub_core::sync::PropagatePlan,
    delete_confirmed: bool,
) -> Result<agenthub_core::sync::SyncReport, String> {
    agenthub_core::sync::apply_skill_sync(&reg, &source_agent, &skill_name, &scope, &plan, delete_confirmed)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn propagate_mcp(
    reg: State<Registry>,
    source_agent: String,
    name: String,
    target_agents: Vec<String>,
) -> Result<Vec<agenthub_core::model::DeployResult>, String> {
    Ok(agenthub_core::sync::propagate_mcp(&reg, &source_agent, &name, &target_agents))
}

/* ---------- AI 工具箱（P2，刻意做轻：卡片+笔记） ---------- */

#[tauri::command]
pub fn fetch_url_metadata(url: String) -> Result<serde_json::Value, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL 必须以 http(s):// 开头".into());
    }
    let (title, description) =
        agenthub_core::market::fetch_url_metadata(&url).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "title": title, "description": description }))
}

/* ---------- Skill 版本与更新 ---------- */

#[tauri::command]
pub fn check_library_update(name: String) -> Result<agenthub_core::updater::UpdateCheck, String> {
    agenthub_core::updater::check_update(&agenthub_core::util::app_data_dir(), &name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_library_update(
    reg: State<Registry>,
    name: String,
    sync_agents: bool,
    delete_confirmed: bool,
) -> Result<agenthub_core::updater::UpdateApplyReport, String> {
    agenthub_core::updater::apply_update(
        &agenthub_core::util::app_data_dir(),
        &name,
        &reg,
        sync_agents,
        delete_confirmed,
    )
    .map_err(|e| e.to_string())
}

/* ---------- 收藏集 ---------- */

#[tauri::command]
pub fn list_collection() -> Result<Vec<agenthub_core::collection::CollectionEntry>, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    agenthub_core::collection::list_items(&store).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_curated() -> Result<Vec<agenthub_core::collection::CollectionEntry>, String> {
    Ok(agenthub_core::collection::list_curated())
}

#[tauri::command]
pub fn add_collection_item(
    entry: agenthub_core::collection::CollectionEntry,
) -> Result<i64, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    agenthub_core::collection::add_item(&store, &entry).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_collection_item(
    entry: agenthub_core::collection::CollectionEntry,
) -> Result<(), String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    agenthub_core::collection::update_item(&store, &entry).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_collection_item(id: i64) -> Result<(), String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    agenthub_core::collection::delete_item(&store, id).map_err(|e| e.to_string())
}

/* ---------- 配置 Profile ---------- */

#[tauri::command]
pub fn list_profiles() -> Result<Vec<agenthub_core::model::ProfileMeta>, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    Ok(store
        .list_profiles()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(id, name)| agenthub_core::model::ProfileMeta { id, name })
        .collect())
}

#[tauri::command]
pub fn list_profile_items(
    profile_id: i64,
) -> Result<Vec<agenthub_core::model::ProfileItem>, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    Ok(store
        .list_profile_items(profile_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(id, kind, ref_name, def_json)| agenthub_core::model::ProfileItem {
            id,
            kind,
            ref_name,
            def: serde_json::from_str(&def_json).unwrap_or(serde_json::Value::Null),
        })
        .collect())
}

#[tauri::command]
pub fn create_profile(
    name: String,
    skills: Vec<String>,
    mcps: Vec<McpEntry>,
) -> Result<i64, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    let pid = store.add_profile(&name).map_err(|e| e.to_string())?;
    for s in &skills {
        store
            .add_profile_item(pid, "skill", s, "{}")
            .map_err(|e| e.to_string())?;
    }
    for m in &mcps {
        store
            .add_profile_item(pid, "mcp", &m.name, &m.raw.to_string())
            .map_err(|e| e.to_string())?;
    }
    Ok(pid)
}

#[tauri::command]
pub fn delete_profile(id: i64) -> Result<(), String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    store.delete_profile(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_profile(
    reg: State<Registry>,
    profile_id: i64,
    agent_ids: Vec<String>,
    overwrite: bool,
) -> Result<Vec<agenthub_core::model::ProfileApplyResult>, String> {
    let store = agenthub_core::store::Store::open_default().map_err(|e| e.to_string())?;
    agenthub_core::profiles::apply(
        &reg,
        &store,
        &agenthub_core::util::app_data_dir(),
        profile_id,
        &agent_ids,
        overwrite,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_custom_agents() -> Result<String, String> {
    Ok(agenthub_core::store::Store::open_default()
        .ok()
        .and_then(|s| s.get_setting(agenthub_core::registry::CUSTOM_AGENTS_KEY).ok().flatten())
        .unwrap_or_else(|| "[]".into()))
}

#[tauri::command]
pub fn set_custom_agents(raw: String) -> Result<(), String> {
    let agents: Vec<agenthub_core::model::AgentDescriptor> =
        serde_json::from_str(&raw).map_err(|e| format!("自定义 Agent JSON 无效: {e}"))?;
    let builtins: std::collections::HashSet<&str> = agenthub_core::registry::descriptors()
        .iter().map(|a| a.id.as_str()).collect();
    let mut ids = std::collections::HashSet::new();
    for agent in &agents {
        if agent.id.trim().is_empty() || agent.name.trim().is_empty() {
            return Err("Agent id 和名称不能为空".into());
        }
        if !ids.insert(agent.id.as_str()) || builtins.contains(agent.id.as_str()) {
            return Err(format!("Agent id 重复或与内置 Agent 冲突: {}", agent.id));
        }
        if agent.mcp_config_paths.windows.is_empty() && agent.mcp_config_paths.macos.is_empty() && agent.mcp_config_paths.linux.is_empty() {
            return Err(format!("{} 至少要配置一个 MCP 配置路径", agent.name));
        }
        if agent.mcp_format != "json-map" && agent.mcp_format != "dsh-array" {
            return Err(format!("{} 的 MCP 格式不支持：{}（可选 json-map / dsh-array）", agent.name, agent.mcp_format));
        }
    }
    let normalized = serde_json::to_string_pretty(&agents).map_err(|e| e.to_string())?;
    agenthub_core::store::Store::open_default()
        .map_err(|e| e.to_string())?
        .set_setting(agenthub_core::registry::CUSTOM_AGENTS_KEY, &normalized)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_path_overrides() -> Result<String, String> {
    Ok(agenthub_core::store::Store::open_default()
        .ok()
        .and_then(|s| {
            s.get_setting(agenthub_core::registry::OVERRIDES_KEY)
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| "{}".into()))
}

#[tauri::command]
pub fn set_path_overrides(raw: String) -> Result<(), String> {
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| format!("JSON 无效: {e}"))?;
    if !v.is_object() {
        return Err("必须是一个 JSON 对象，键为 Agent id".into());
    }
    for (agent_id, spec) in v.as_object().unwrap() {
        if !spec.is_object() {
            return Err(format!("{agent_id} 的覆写必须是对象"));
        }
        for key in spec.as_object().unwrap().keys() {
            if key != "mcpConfigPaths" && key != "skillDirs" {
                return Err(format!("{agent_id} 不支持字段 {key}（仅 mcpConfigPaths / skillDirs）"));
            }
        }
    }
    agenthub_core::store::Store::open_default()
        .map_err(|e| e.to_string())?
        .set_setting(agenthub_core::registry::OVERRIDES_KEY, raw.trim())
        .map_err(|e| e.to_string())
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
