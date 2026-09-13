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

    println!("\n=== Skill 条目（前 12 个） ===");
    let skills = reg.all_skills();
    for s in skills.iter().take(12) {
        println!(
            "{:<12} {:<20} {:<8} {}",
            s.agent_id,
            s.name,
            if s.scope == "user" { "user" } else { "project" },
            s.description.as_deref().unwrap_or("-")
        );
    }
    println!("共 {} 个 skill；中央库现有 {} 个条目", skills.len(), {
        agenthub_core::library::list_library().len()
    });
}
