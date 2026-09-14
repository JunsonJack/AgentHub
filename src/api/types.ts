// 与 agenthub-core 的 serde camelCase 模型一一对应
export interface AgentDescriptor {
  id: string;
  name: string;
  kind: "cli" | "ide" | "desktop";
  mcpConfigPaths: { windows: string[]; macos: string[]; linux: string[] };
  skillDirs: { windows: string[]; macos: string[]; linux: string[] };
  mcpFormat: string;
  reload: string;
}


export interface AgentStatus {
  id: string;
  name: string;
  kind: "cli" | "ide" | "desktop";
  installed: boolean;
  foundPaths: string[];
  mcpCount: number | null;
  skillCount: number | null;
  healthNote: string | null;
}

export interface McpEntry {
  agentId: string;
  name: string;
  /** "global" 或 "project:<路径>" */
  scope: string;
  /** stdio | http | sse | unknown */
  transport: string;
  command: string | null;
  args: string[];
  url: string | null;
  raw: Record<string, unknown>;
}

/** 下发用受控定义（未知字段进 extra，serde flatten） */
export interface McpServerDef {
  command?: string;
  args?: string[];
  url?: string;
  env?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface WriteReport {
  backupPath: string | null;
  changed: boolean;
}

export interface DeployResult {
  agentId: string;
  ok: boolean;
  error: string | null;
  backupPath: string | null;
}

export interface SkillEntry {
  agentId: string;
  name: string;
  /** "user" 或 "project:<路径>" */
  scope: string;
  dir: string;
  description: string | null;
  hasSkillMd: boolean;
}

export interface LibraryItem {
  name: string;
  kind: string;
  sourceAgent: string | null;
  sourcePath: string | null;
  description: string | null;
  adoptedAt: number;
  fileCount: number;
}

export interface AdoptReport {
  dryRun: boolean;
  skillName: string;
  targetDir: string;
  files: string[];
  conflict: boolean;
}

export interface SnapshotMeta {
  id: string;
  fileName: string;
  originalPath: string;
  createdAt: number;
}

/* ---------- 市场与收藏 ---------- */

export interface MarketSkill {
  /** skills.sh: "owner/repo/slug"；skillsmp: 平台 id */
  id: string;
  name: string;
  /** "skills.sh" | "skillsmp" */
  market: string;
  source: string | null;
  author: string | null;
  description: string | null;
  installs: number | null;
  stars: number | null;
  githubUrl: string | null;
}

export interface MarketPreview {
  skillName: string;
  description: string | null;
  files: string[];
}

export interface InstallOutcome {
  adopt: AdoptReport;
  deploys: DeployResult[];
}

export interface KeyStatus {
  set: boolean;
  masked: string | null;
}

export interface HealthIssue {
  agentId: string;
  /** "error" | "warning" */
  severity: string;
  code: string;
  message: string;
  path: string | null;
}

export interface TrashItem {
  id: string;
  originalPath: string;
  agentId: string;
  name: string;
  deletedAt: number;
}

export interface DisabledRecord {
  id: number;
  agentId: string;
  name: string;
  scope: string;
  def: Record<string, unknown>;
  disabledAt: number;
}

export interface ConnectivityResult {
  /** "ok" | "failed" */
  status: string;
  latencyMs: number;
  serverName: string | null;
  serverVersion: string | null;
  error: string | null;
}

export interface CollectionEntry {
  id: number | null;
  kind: string;
  name: string;
  source: string;
  tags: string[];
  note: string;
  stars: number;
  builtIn: boolean;
}

export interface ProfileMeta {
  id: number;
  name: string;
}

export interface ProfileItem {
  id: number;
  kind: "skill" | "mcp";
  refName: string;
  def: Record<string, unknown>;
}

export interface ProfileApplyResult {
  kind: string;
  name: string;
  agentId: string;
  ok: boolean;
  error: string | null;
}

export interface PropagatePlan {
  targetAgent: string;
  targetDir: string;
  copy: string[];
  deletions: string[];
  targetAbsent: boolean;
  identical: boolean;
}

export interface SyncReport {
  targetAgent: string;
  copied: number;
  deleted: number;
  heldBackDeletions: string[];
  backupDir: string | null;
}

export interface UpdateCheck {
  name: string;
  source: string;
  checkable: boolean;
  hasUpdates: boolean;
  identical: boolean;
  incoming: string[];
  changed: string[];
  upstreamRemoved: string[];
  error: string | null;
}

export interface UpdateApplyReport {
  name: string;
  updatedLibrary: boolean;
  fileCount: number;
  synced: SyncReport[];
  error: string | null;
}
