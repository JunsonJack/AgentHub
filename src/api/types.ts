// 与 agenthub-core 的 serde camelCase 模型一一对应
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
