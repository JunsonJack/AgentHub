import { defineStore } from "pinia";
import { listAgents, listMcpAll } from "../api";
import type { AgentStatus, McpEntry } from "../api/types";

export const useAgentsStore = defineStore("agents", {
  state: () => ({
    agents: [] as AgentStatus[],
    mcp: [] as McpEntry[],
    loading: false,
    loaded: false,
    error: "" as string,
  }),
  getters: {
    installedCount: (s) => s.agents.filter((a) => a.installed).length,
  },
  actions: {
    async fetchAll() {
      this.loading = true;
      this.error = "";
      try {
        this.agents = await listAgents();
        this.mcp = await listMcpAll();
        this.loaded = true;
      } catch (e) {
        this.error = String(e);
        throw e;
      } finally {
        this.loading = false;
      }
    },
  },
});
