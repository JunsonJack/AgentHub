mod commands;

use agenthub_core::registry::Registry;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 外链 / 本地路径交由系统默认应用处理：webview 只有一个窗口且无后退键，
        // 任何在应用内跳转的外链都会把用户锁死在别人页面上。
        .plugin(tauri_plugin_opener::init())
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
            commands::disable_mcp,
            commands::list_disabled_mcp,
            commands::enable_mcp,
            commands::test_mcp,
            commands::test_mcp_def,
            commands::get_custom_agents,
            commands::set_custom_agents,
            commands::list_collection,
            commands::list_curated,
            commands::add_collection_item,
            commands::update_collection_item,
            commands::delete_collection_item,
            commands::list_profiles,
            commands::list_profile_items,
            commands::create_profile,
            commands::delete_profile,
            commands::apply_profile,
            commands::check_library_update,
            commands::apply_library_update,
            commands::plan_skill_sync,
            commands::apply_skill_sync,
            commands::propagate_mcp,
            commands::fetch_url_metadata,
            commands::list_skills,
            commands::adopt_skill,
            commands::list_library,
            commands::read_library_skill,
            commands::import_skill_folder,
            commands::list_snapshots,
            commands::remove_skill,
            commands::list_trash,
            commands::restore_trash,
            commands::rollback_snapshot,
            commands::prune_snapshots,
            commands::read_skill_md,
            commands::get_path_overrides,
            commands::set_path_overrides,
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
