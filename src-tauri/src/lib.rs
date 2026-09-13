mod commands;

use agenthub_core::registry::Registry;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let registry = Registry::load()
                .map_err(|e| format!("装载 Agent 注册表失败: {e}"))?;
            app.manage(registry);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_agents,
            commands::list_mcp_all,
            commands::list_health_issues,
            commands::deploy_mcp,
            commands::remove_mcp,
            commands::list_skills,
            commands::adopt_skill,
            commands::list_library,
            commands::read_library_skill,
            commands::list_snapshots,
            commands::remove_skill,
            commands::list_trash,
            commands::restore_trash,
            commands::rollback_snapshot,
            commands::read_skill_md,
            commands::market_search,
            commands::market_preview,
            commands::market_install_skills_sh,
            commands::market_install_git,
            commands::deploy_library_skill,
            commands::skillsmp_key_status,
            commands::skillsmp_set_key,
            commands::skillsmp_clear_key
        ])
        .run(tauri::generate_context!())
        .expect("AgentHub 启动失败");
}
