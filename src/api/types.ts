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
