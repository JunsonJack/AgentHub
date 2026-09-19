mod commands;

use agenthub_core::registry::Registry;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    // 双开保护：第二个实例启动时聚焦已有窗口后退出，
    // 避免两个进程同时操作同一份 SQLite / Agent 配置互相覆盖。
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }));
    }
    builder
        // 外链 / 本地路径交由系统默认应用处理：webview 只有一个窗口且无后退键，
        // 任何在应用内跳转的外链都会把用户锁死在别人页面上。
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // 窗口位置 / 大小记忆（崩溃或断电也不丢上次布局）
        .plugin(tauri_plugin_window_state::Builder::new().build())
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
            commands::skillsmp_clear_key,
            commands::list_secrets,
            commands::save_secret,
            commands::get_secret,
            commands::delete_secret,
            commands::export_secrets,
            commands::import_secrets,
            commands::scan_env_secrets,
            commands::parse_bundle,
            commands::install_bundle,
            commands::export_collection,
            commands::import_collection
        ])
        .run(tauri::generate_context!())
        .expect("AgentHub 启动失败");
}
