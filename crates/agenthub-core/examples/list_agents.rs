//! 冒烟测试：直接读本机真实 Agent 配置。
//! 运行：cargo run -p agenthub-core --example list_agents

use agenthub_core::registry::Registry;

fn main() {
    let reg = Registry::load().expect("装载注册表");

    println!("=== Agent 状态 ===");
    for st in reg.statuses() {
        println!(
            "{:<14} installed={:<5} mcp={:<4} skills={:<4}",
            st.name,
            st.installed,
            st.mcp_count.map_or("-".into(), |n| n.to_string()),
            st.skill_count.map_or("-".into(), |n| n.to_string()),
        );
        for p in &st.found_paths {
            println!("    path: {p}");
        }
        if let Some(note) = &st.health_note {
            println!("    !! {note}");
        }
    }

    println!("\n=== MCP 条目（不展示 env 值） ===");
    for e in reg.all_mcp() {
        let target = e.command.as_deref().or(e.url.as_deref()).unwrap_or("-");
        println!("{:<12} {:<24} {:<12} {}", e.agent_id, e.name, e.scope, target);
    }
}
