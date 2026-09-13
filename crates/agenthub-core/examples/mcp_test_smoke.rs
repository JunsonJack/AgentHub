//! 连通性冒烟：对本机真实 MCP server 逐个握手（只读，不改任何配置）。
//! 运行：cargo run -p agenthub-core --example mcp_test_smoke

use agenthub_core::registry::Registry;
use agenthub_core::runner;

fn main() {
    let reg = Registry::load().expect("装载注册表");
    let mut ok = 0;
    let mut fail = 0;
    let entries: Vec<_> = reg
        .all_mcp()
        .into_iter()
        .filter(|e| e.scope == "global")
        .collect();
    println!("共 {} 个条目待测", entries.len());
    for e in entries {
        eprintln!(
            "testing {:<12} {:<24} ...",
            e.agent_id, e.name
        );
        let r = runner::test_entry(&reg, &e.agent_id, &e.name, &e.scope);
        match r.status.as_str() {
            "ok" => {
                ok += 1;
                println!(
                    "{:<12} {:<24} OK  {:>5}ms  {} {}",
                    e.agent_id,
                    e.name,
                    r.latency_ms,
                    r.server_name.as_deref().unwrap_or("-"),
                    r.server_version.as_deref().unwrap_or("")
                );
            }
            _ => {
                fail += 1;
                println!(
                    "{:<12} {:<24} FAIL {:>5}ms  {}",
                    e.agent_id,
                    e.name,
                    r.latency_ms,
                    r.error.as_deref().unwrap_or("-")
                );
            }
        }
    }
    println!("结果：{ok} 连通 / {fail} 失败");
}
