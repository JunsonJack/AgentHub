<script setup lang="ts">
import { onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import { ElMessage } from "element-plus";
import { Refresh } from "@element-plus/icons-vue";
import { useAgentsStore } from "../stores/agents";
import { listHealthIssues } from "../api";
import type { HealthIssue } from "../api/types";

const store = useAgentsStore();
const { agents, loading, loaded, error, installedCount } = storeToRefs(store);
const issues = ref<HealthIssue[]>([]);

async function refresh() {
  try {
    await store.fetchAll();
    issues.value = await listHealthIssues();
  } catch {
    ElMessage.error(error.value || "读取 Agent 状态失败");
  }
}

const kindLabel: Record<string, string> = { cli: "CLI", ide: "IDE 插件", desktop: "桌面应用" };

onMounted(refresh);
</script>

<template>
  <div>
    <el-row :gutter="16" class="stat-row">
      <el-col :span="8">
        <el-card shadow="never">
          <div class="stat-num">{{ installedCount }} / {{ agents.length }}</div>
          <div class="stat-label">本机识别到的 Agent</div>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card shadow="never">
          <div class="stat-num">{{ store.mcp.length }}</div>
          <div class="stat-label">MCP server 条目（跨 Agent 汇总）</div>
        </el-card>
      </el-col>
      <el-col :span="8">
        <el-card shadow="never">
          <div class="stat-num">{{ issues.filter((i) => i.severity === "error").length }}</div>
          <div class="stat-label">错误级健康问题（解析失败 / 非法条目）</div>
        </el-card>
      </el-col>
    </el-row>

    <div v-if="issues.length" class="section-head">
      <span class="section-title">待处理提醒</span>
    </div>
    <el-alert
      v-for="(i, idx) in issues"
      :key="idx"
      :title="i.message"
      :type="i.severity === 'error' ? 'error' : 'warning'"
      :closable="false"
      class="issue-alert"
    />

    <div class="section-head">
      <span class="section-title">Agent 卡片墙</span>
      <el-button :icon="Refresh" size="small" :loading="loading" @click="refresh">刷新</el-button>
    </div>

    <el-row :gutter="16" v-loading="loading && !loaded">
      <el-col v-for="a in agents" :key="a.id" :span="8" class="card-col">
        <el-card shadow="hover">
          <template #header>
            <div class="card-head">
              <span class="agent-name">{{ a.name }}</span>
              <el-space>
                <el-tag size="small" effect="plain">{{ kindLabel[a.kind] ?? a.kind }}</el-tag>
                <el-tag size="small" :type="a.installed ? 'success' : 'info'">
                  {{ a.installed ? "已安装" : "未检测到" }}
                </el-tag>
              </el-space>
            </div>
          </template>
          <div class="card-line"><span class="k">MCP</span><span>{{ a.mcpCount ?? "—" }} 个</span></div>
          <div class="card-line"><span class="k">Skill</span><span>{{ a.skillCount ?? "—" }} 个</span></div>
          <div v-if="a.foundPaths.length" class="paths">
            <div v-for="p in a.foundPaths" :key="p" class="path-item">{{ p }}</div>
          </div>
          <el-alert
            v-if="a.healthNote"
            :title="a.healthNote"
            type="warning"
            :closable="false"
            class="health-alert"
          />
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<style scoped>
.stat-row { margin-bottom: 20px; }
.stat-num { font-size: 26px; font-weight: 700; }
.stat-label { font-size: 12px; color: var(--el-text-color-secondary); margin-top: 4px; }
.section-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
.section-title { font-weight: 600; }
.card-col { margin-bottom: 16px; }
.card-head { display: flex; justify-content: space-between; align-items: center; }
.agent-name { font-weight: 600; }
.card-line { display: flex; justify-content: space-between; font-size: 13px; padding: 3px 0; }
.card-line .k { color: var(--el-text-color-secondary); }
.paths { margin-top: 8px; }
.path-item {
  font-size: 11px; color: var(--el-text-color-secondary);
  font-family: Consolas, monospace;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.health-alert { margin-top: 10px; }
.issue-alert { margin-bottom: 8px; }
</style>
