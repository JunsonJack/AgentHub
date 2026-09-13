import { invoke } from "@tauri-apps/api/core";
import type {
  AdoptReport,
  AgentStatus,
  DeployResult,
  HealthIssue,
  InstallOutcome,
  KeyStatus,
  LibraryItem,
  MarketPreview,
  MarketSkill,
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

export function listHealthIssues(): Promise<HealthIssue[]> {
  return invoke<HealthIssue[]>("list_health_issues");
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

/* ---------- 市场与收藏 ---------- */

export function marketSearch(
  source: string,
  q: string,
  limit = 20
): Promise<MarketSkill[]> {
  return invoke<MarketSkill[]>("market_search", { source, q, limit });
}

export function marketPreview(id: string): Promise<MarketPreview> {
  return invoke<MarketPreview>("market_preview", { id });
}

export function marketInstallSkillsSh(
  id: string,
  agentIds: string[],
  overwrite: boolean
): Promise<InstallOutcome> {
  return invoke<InstallOutcome>("market_install_skills_sh", { id, agentIds, overwrite });
}

export function marketInstallGit(
  url: string,
  agentIds: string[],
  overwrite: boolean
): Promise<InstallOutcome> {
  return invoke<InstallOutcome>("market_install_git", { url, agentIds, overwrite });
}

export function deployLibrarySkill(
  name: string,
  agentIds: string[],
  overwrite: boolean
): Promise<DeployResult[]> {
  return invoke<DeployResult[]>("deploy_library_skill", { name, agentIds, overwrite });
}

export function skillsmpKeyStatus(): Promise<KeyStatus> {
  return invoke<KeyStatus>("skillsmp_key_status");
}

export function skillsmpSetKey(key: string): Promise<void> {
  return invoke<void>("skillsmp_set_key", { key });
}

export function skillsmpClearKey(): Promise<void> {
  return invoke<void>("skillsmp_clear_key");
}
