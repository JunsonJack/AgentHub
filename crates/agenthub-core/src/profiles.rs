//! 配置 Profile 应用（P1 第 8 项）：skill 走 deploy_skill；MCP 走 deploy_mcp
//! 且应用与同步引擎一致的密钥保护（目标已有 env 密钥保留本地值，缺失填占位符）。

use crate::model::{ProfileApplyResult, ProfileItem, McpServerDef};
use crate::registry::Registry;
use crate::store::Store;

pub fn apply(
    reg: &Registry,
    store: &Store,
    data_root: &std::path::Path,
    profile_id: i64,
    agent_ids: &[String],
    overwrite: bool,
) -> crate::error::Result<Vec<ProfileApplyResult>> {
    let items: Vec<ProfileItem> = store
        .list_profile_items(profile_id)?
        .into_iter()
        .map(|(id, kind, ref_name, def_json)| ProfileItem {
            id,
            kind,
            ref_name,
            def: serde_json::from_str(&def_json).unwrap_or(serde_json::Value::Null),
        })
        .collect();

    let all_mcp = reg.all_mcp();
    let mut out = vec![];

    for item in items {
        match item.kind.as_str() {
            "skill" => {
                for r in reg.deploy_skill_in(data_root, &item.ref_name, agent_ids, overwrite) {
                    out.push(ProfileApplyResult {
                        kind: "skill".into(),
                        name: item.ref_name.clone(),
                        agent_id: r.agent_id,
                        ok: r.ok,
                        error: r.error,
                    });
                }
            }
            "mcp" => {
                let def: McpServerDef = match serde_json::from_value(item.def.clone()) {
                    Ok(d) => d,
                    Err(e) => {
                        out.push(ProfileApplyResult {
                            kind: "mcp".into(),
                            name: item.ref_name.clone(),
                            agent_id: agent_ids.join(","),
                            ok: false,
                            error: Some(format!("定义解析失败: {e}")),
                        });
                        continue;
                    }
                };
                for agent_id in agent_ids {
                    // 密钥保护：按目标本地 env 合并（与 sync::propagate_mcp 同规则）
                    let target_env = all_mcp
                        .iter()
                        .find(|e| e.agent_id == *agent_id && e.name == item.ref_name && e.scope == "global")
                        .and_then(|e| e.raw.get("env").and_then(|v| v.as_object()).cloned());
                    let mut per_target = def.clone();
                    if let Some(src_env) = &def.env {
                        per_target.env = Some(crate::sync::merge_env_for_target(src_env, target_env.as_ref()));
                    }
                    for r in reg.deploy_mcp(std::slice::from_ref(agent_id), &item.ref_name, &per_target, None) {
                        out.push(ProfileApplyResult {
                            kind: "mcp".into(),
                            name: item.ref_name.clone(),
                            agent_id: r.agent_id,
                            ok: r.ok,
                            error: r.error,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}
