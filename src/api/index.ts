import { invoke } from "@tauri-apps/api/core";
import type {
  AdoptReport,
  AgentStatus,
  DeployResult,
  LibraryItem,
  McpEntry,
  McpServerDef,
  SkillEntry,
  SnapshotMeta,
  WriteReport,
} from "./types";

export function listAgents(): Promise<AgentStatus[]> {
  return invoke<AgentStatus[]>("list_agents");
}

export function listMcpAll(): Promise<McpEntry[]> {
  return invoke<McpEntry[]>("list_mcp_all");
}

export function deployMcp(
  agentIds: string[],
  name: string,
  def: McpServerDef
): Promise<DeployResult[]> {
  return invoke<DeployResult[]>("deploy_mcp", {
    agentIds,
    name,
    def,
  });
}

export function removeMcp(
  agentId: string,
  name: string,
  scope: string
): Promise<WriteReport> {
  return invoke<WriteReport>("remove_mcp", { agentId, name, scope });
}

export function listSkills(): Promise<SkillEntry[]> {
  return invoke<SkillEntry[]>("list_skills");
}

export function adoptSkill(
  agentId: string,
  skillName: string,
  scope: string,
  dryRun: boolean
): Promise<AdoptReport> {
  return invoke<AdoptReport>("adopt_skill", { agentId, skillName, scope, dryRun });
}

export function listLibrary(): Promise<LibraryItem[]> {
  return invoke<LibraryItem[]>("list_library");
}

export function readLibrarySkill(name: string): Promise<string> {
  return invoke<string>("read_library_skill", { name });
}

export function listSnapshots(): Promise<SnapshotMeta[]> {
  return invoke<SnapshotMeta[]>("list_snapshots");
}

export function rollbackSnapshot(id: string, fileName: string): Promise<string> {
  return invoke<string>("rollback_snapshot", { id, fileName });
}

export function readSkillMd(dir: string): Promise<string> {
  return invoke<string>("read_skill_md", { dir });
}
