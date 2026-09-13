<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import { ElMessage } from "element-plus";
import { Refresh, Search } from "@element-plus/icons-vue";
import { useAgentsStore } from "../stores/agents";

const store = useAgentsStore();
const { mcp, agents, loading, loaded, error } = storeToRefs(store);

const keyword = ref("");
const agentFilter = ref("");

const agentName = (id: string) => agents.value.find((a) => a.id === id)?.name ?? id;

const filtered = computed(() =>
  mcp.value.filter((e) => {
    const okAgent = !agentFilter.value || e.agentId === agentFilter.value;
    const kw = keyword.value.trim().toLowerCase();
    const okKw =
      !kw ||
      e.name.toLowerCase().includes(kw) ||
      (e.command ?? "").toLowerCase().includes(kw) ||
      (e.url ?? "").toLowerCase().includes(kw);
    return okAgent && okKw;
  })
);

const transportTag = (t: string) => (t === "stdio" ? "success" : t === "unknown" ? "info" : "warning");
const scopeLabel = (s: string) => (s === "global" ? "全局" : s.startsWith("project:") ? "项目" : s);

async function refresh() {
  try {
    await store.fetchAll();
  } catch {
    ElMessage.error(error.value || "读取 MCP 配置失败");
  }
}

onMounted(refresh);
</script>

<template>
  <div>
    <div class="toolbar">
      <el-input v-model="keyword" :prefix-icon="Search" placeholder="搜索 server 名 / command / url" clearable class="kw" />
      <el-select v-model="agentFilter" placeholder="全部 Agent" clearable class="agent-select">
        <el-option v-for="a in agents" :key="a.id" :label="a.name" :value="a.id" />
      </el-select>
      <el-button :icon="Refresh" :loading="loading" @click="refresh">刷新</el-button>
    </div>

    <el-alert
      v-if="loaded && mcp.length === 0"
      title="没有读到任何 MCP 配置"
      description="各 Agent 的配置文件不存在或为空。Connector v0 支持 Claude Code / ZCode / Codex / Claude Desktop / Gemini 的读取。"
      type="info"
      :closable="false"
    />

    <el-table :data="filtered" v-loading="loading && !loaded" stripe class="mcp-table">
      <el-table-column label="Server" min-width="180">
        <template #default="{ row }">
          <span class="server-name">{{ row.name }}</span>
        </template>
      </el-table-column>
      <el-table-column label="Agent" width="130">
        <template #default="{ row }">{{ agentName(row.agentId) }}</template>
      </el-table-column>
      <el-table-column label="作用域" width="90">
        <template #default="{ row }">
          <el-tag size="small" :type="row.scope === 'global' ? 'success' : 'warning'" effect="plain">
            {{ scopeLabel(row.scope) }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="传输" width="90">
        <template #default="{ row }">
          <el-tag size="small" :type="transportTag(row.transport)" effect="plain">{{ row.transport }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="启动命令 / URL" min-width="260">
        <template #default="{ row }">
          <code class="cmd">{{ row.command ?? row.url ?? "—" }}</code>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<style scoped>
.toolbar { display: flex; gap: 10px; margin-bottom: 14px; }
.kw { width: 320px; }
.agent-select { width: 180px; }
.server-name { font-weight: 600; }
.mcp-table { background: #fff; }
.cmd {
  font-size: 12px; font-family: Consolas, monospace;
  background: var(--el-fill-color); padding: 2px 6px; border-radius: 4px;
}
</style>
