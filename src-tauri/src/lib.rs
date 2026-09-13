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
            commands::list_mcp_all
        ])
        .run(tauri::generate_context!())
        .expect("AgentHub 启动失败");
}
