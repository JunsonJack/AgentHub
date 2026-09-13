import { invoke } from "@tauri-apps/api/core";
import type { AgentStatus, McpEntry } from "./types";

export function listAgents(): Promise<AgentStatus[]> {
  return invoke<AgentStatus[]>("list_agents");
}

export function listMcpAll(): Promise<McpEntry[]> {
  return invoke<McpEntry[]>("list_mcp_all");
}
