<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { DeleteLocation, Key, Refresh, RefreshLeft } from "@element-plus/icons-vue";
import { useTheme, type ThemeMode } from "../composables/theme";
import {
  getCustomAgents,
  getPathOverrides,
  listSnapshots,
  listTrash,
  pruneSnapshots,
  restoreTrash,
  rollbackSnapshot,
  setCustomAgents,
  setPathOverrides,
  skillsmpClearKey,
  skillsmpKeyStatus,
  skillsmpSetKey,
} from "../api";
import type { AgentDescriptor, SnapshotMeta, TrashItem } from "../api/types";

const snapshots = ref<SnapshotMeta[]>([]);
const loading = ref(false);
const filter = ref("");
const snapshotPage = ref(1);
const snapshotPageSize = 10;
const { themeMode, setTheme } = useTheme();

const filtered = computed(() =>
  snapshots.value.filter(
    (s) =>
      !filter.value.trim() ||
      s.originalPath.toLowerCase().includes(filter.value.trim().toLowerCase()) ||
      s.fileName.toLowerCase().includes(filter.value.trim().toLowerCase())
  )
);

const pagedSnapshots = computed(() => filtered.value.slice((snapshotPage.value - 1) * snapshotPageSize, snapshotPage.value * snapshotPageSize));

watch(filter, () => { snapshotPage.value = 1; });

function fmtTime(ms: number): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleString("zh-CN", { hour12: false });
}

async function refresh() {
  loading.value = true;
  try {
    snapshots.value = await listSnapshots();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    loading.value = false;
  }
}

async function onRollback(s: SnapshotMeta) {
  try {
    await ElMessageBox.confirm(
      `把快照恢复到原位置：\n${s.originalPath}\n\n回滚前会对当前文件再自动拍一次快照，回滚本身可撤销。`,
      "确认回滚",
      { type: "warning", confirmButtonText: "回滚", cancelButtonText: "取消" }
    );
  } catch {
    return;
  }
  try {
    const restored = await rollbackSnapshot(s.id, s.fileName);
    ElMessage.success(`已恢复：${restored}`);
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

/* ---------- SkillsMP API 密钥 ---------- */

const keyStatus = ref<{ set: boolean; masked: string | null }>({ set: false, masked: null });
const keyInput = ref("");
const keySaving = ref(false);

async function refreshKey() {
  try {
    keyStatus.value = await skillsmpKeyStatus();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function saveKey() {
  if (!keyInput.value.trim()) {
    ElMessage.warning("请输入密钥（skillsmp.com/docs/api 生成）");
    return;
  }
  keySaving.value = true;
  try {
    await skillsmpSetKey(keyInput.value.trim());
    keyInput.value = "";
    ElMessage.success("密钥已保存（仅存本机）");
    await refreshKey();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    keySaving.value = false;
  }
}

async function clearKey() {
  try {
    await skillsmpClearKey();
    ElMessage.success("已清除");
    await refreshKey();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

onMounted(() => {
  refresh();
  refreshKey();
  refreshTrash();
  refreshOverrides();
  refreshCustomAgents();
});

/* ---------- 回收站 ---------- */

const trashItems = ref<TrashItem[]>([]);
const trashLoading = ref(false);

const customAgentsRaw = ref("[]");
const customAgentsSaving = ref(false);

async function refreshCustomAgents() {
  try {
    customAgentsRaw.value = await getCustomAgents();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function saveCustomAgents() {
  customAgentsSaving.value = true;
  try {
    const agents = JSON.parse(customAgentsRaw.value) as AgentDescriptor[];
    if (!Array.isArray(agents)) throw new Error("必须是数组");
    await setCustomAgents(JSON.stringify(agents));
    ElMessage.success("自定义 Agent 已保存，重启应用后生效");
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    customAgentsSaving.value = false;
  }
}

/* ---------- Agent 路径覆写 ---------- */
const overridesRaw = ref("{}");
const overridesSaving = ref(false);

async function refreshOverrides() {
  try {
    overridesRaw.value = await getPathOverrides();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function saveOverrides() {
  overridesSaving.value = true;
  try {
    await setPathOverrides(overridesRaw.value);
    ElMessage.success("已保存，重启应用后生效");
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    overridesSaving.value = false;
  }
}

async function refreshTrash() {
  trashLoading.value = true;
  try {
    trashItems.value = await listTrash();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    trashLoading.value = false;
  }
}

async function onRestore(t: TrashItem) {
  try {
    const restored = await restoreTrash(t.id);
    ElMessage.success(`已恢复：${restored}`);
    await refreshTrash();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function onPrune() {
  try {
    await ElMessageBox.confirm(
      "清理最旧的快照，仅保留最近 50 组。清理不可恢复，确定继续？",
      "清理快照",
      { type: "warning", confirmButtonText: "清理", cancelButtonText: "取消" }
    );
  } catch {
    return;
  }
  try {
    const removed = await pruneSnapshots(50);
    ElMessage.success(removed ? `已清理 ${removed} 组旧快照` : "没有需要清理的快照");
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}
</script>

<template>
  <div>
    <el-descriptions :column="1" border class="mb">
      <el-descriptions-item label="数据目录">
        <code class="cmd">%APPDATA%\AgentHub —— 快照、中央库、SQLite 都在这里</code>
      </el-descriptions-item>
      <el-descriptions-item label="快照策略">
        任何写配置动作前自动快照；回滚前也会对当前文件再快照一次（可撤销）
      </el-descriptions-item>
    </el-descriptions>

    <div class="section-head">
      <span class="section-title">外观</span>
    </div>
    <el-card shadow="never" class="mb appearance-card">
      <div class="appearance-row">
        <div>
          <div class="appearance-title">主题</div>
          <div class="key-note">跟随系统外观，或固定使用日间 / 夜间模式</div>
        </div>
        <el-segmented
          :model-value="themeMode"
          :options="[
            { label: '跟随系统', value: 'system' },
            { label: '日间', value: 'light' },
            { label: '夜间', value: 'dark' },
          ]"
          @change="(value: any) => setTheme(value as ThemeMode)"
        />
      </div>
    </el-card>

    <div class="section-head">
      <span class="section-title">SkillsMP API 密钥</span>
    </div>
    <el-card shadow="never" class="mb">
      <div class="key-row">
        <el-input
          v-model="keyInput"
          type="password"
          show-password
          :prefix-icon="Key"
          placeholder="sk_live_...（在 skillsmp.com/docs/api 生成）"
          class="key-input"
        />
        <el-button type="primary" :loading="keySaving" @click="saveKey">保存</el-button>
        <el-button :disabled="!keyStatus.set" @click="clearKey">清除</el-button>
      </div>
      <div class="key-note">
        <template v-if="keyStatus.set">
          当前密钥：<code class="cmd">{{ keyStatus.masked }}</code>
        </template>
        <template v-else>未设置。匿名可用（每天 50 次搜索）；配置密钥后每天 500 次并启用按技能功能（语义）排序的搜索。</template>
        密钥只保存在本机 SQLite（%APPDATA%\AgentHub），P2 计划迁移到系统钥匙串。
      </div>
    </el-card>

    <div class="section-head">
      <span class="section-title">自定义 Agent</span>
    </div>
    <el-card shadow="never" class="mb">
      <p class="key-note mb">
        不限制 Agent 名单。每个 Agent 的 MCP 格式可选 <code class="cmd">json-map</code> 或 <code class="cmd">dsh-array</code>；保存后重启应用生效。
      </p>
      <el-input v-model="customAgentsRaw" type="textarea" :rows="12" spellcheck="false" class="mono" placeholder="[]" />
      <el-button type="primary" class="mt" :loading="customAgentsSaving" @click="saveCustomAgents">保存自定义 Agent</el-button>
      <el-button class="mt" @click="customAgentsRaw = JSON.stringify([{
        id: 'dsh', name: 'DeepSeek Harness', kind: 'cli',
        mcpConfigPaths: { windows: ['~/.dsh/dsh-mcp.json'], macos: ['~/.dsh/dsh-mcp.json'], linux: ['~/.dsh/dsh-mcp.json'] },
        skillDirs: { windows: [], macos: [], linux: [] }, mcpFormat: 'dsh-array', reload: 'restart'
      }], null, 2)">填入 dsh 示例</el-button>
    </el-card>

    <div class="section-head">
      <span class="section-title">Agent 路径覆写（进阶）</span>
    </div>
    <el-card shadow="never" class="mb">
      <p class="key-note mb">
        当 Agent 装在自定义位置时，可覆写其配置路径与 skill 目录（仅当前操作系统生效，路径支持 ~ 前缀）。
        示例：{"claude-code": {"mcpConfigPaths": ["D:/portable/.claude.json"]}}。
        保存后<b>重启应用</b>生效；留空对象 {} 恢复默认。
      </p>
      <el-input
        v-model="overridesRaw"
        type="textarea"
        :rows="5"
        spellcheck="false"
        class="mono"
        placeholder='{}'
      />
      <el-button type="primary" class="mt" :loading="overridesSaving" @click="saveOverrides">保存覆写</el-button>
    </el-card>

    <div class="section-head">
      <span class="section-title">快照历史（{{ snapshots.length }}）</span>
      <div>
        <el-input v-model="filter" placeholder="按原路径过滤" clearable size="small" class="filter" />
        <el-button size="small" :icon="Refresh" :loading="loading" @click="refresh">刷新</el-button>
        <el-button size="small" type="danger" plain @click="onPrune">清理旧快照（保留 50）</el-button>
      </div>
    </div>

    <div class="history-panel">
      <el-table height="100%" :data="pagedSnapshots" v-loading="loading" stripe>
      <el-table-column label="时间" width="170">
        <template #default="{ row }">{{ fmtTime(row.createdAt) }}</template>
      </el-table-column>
      <el-table-column prop="fileName" label="文件" width="220" />
      <el-table-column label="原路径" min-width="320">
        <template #default="{ row }">
          <code class="cmd">{{ row.originalPath || "（缺 manifest，未知）" }}</code>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="110" fixed="right">
        <template #default="{ row }">
          <el-button
            size="small"
            type="warning"
            plain
            :icon="DeleteLocation"
            :disabled="!row.originalPath"
            @click="onRollback(row)"
          >回滚</el-button>
        </template>
      </el-table-column>
      <template #empty>还没有任何快照——第一次在 MCP 中心保存配置后就会出现</template>
      </el-table>
    </div>
    <el-pagination
      v-model:current-page="snapshotPage"
      :page-size="snapshotPageSize"
      :total="filtered.length"
      layout="total, prev, pager, next"
      background
      class="history-pagination"
    />

    <div class="section-head">
      <span class="section-title">回收站（{{ trashItems.length }}）</span>
      <el-button size="small" :icon="Refresh" :loading="trashLoading" @click="refreshTrash">刷新</el-button>
    </div>
    <div class="history-panel trash-panel">
      <el-table height="100%" :data="trashItems" v-loading="trashLoading" stripe>
      <el-table-column label="删除时间" width="170">
        <template #default="{ row }">{{ fmtTime(row.deletedAt) }}</template>
      </el-table-column>
      <el-table-column prop="name" label="Skill" width="200" />
      <el-table-column prop="agentId" label="Agent" width="130" />
      <el-table-column label="原路径" min-width="320">
        <template #default="{ row }">
          <code class="cmd">{{ row.originalPath }}</code>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="110" fixed="right">
        <template #default="{ row }">
          <el-button size="small" type="success" plain :icon="RefreshLeft" @click="onRestore(row)">恢复</el-button>
        </template>
      </el-table-column>
      <template #empty>回收站是空的——在 Skill 中心删除的 skill 会出现在这里，可随时恢复</template>
      </el-table>
    </div>
  </div>
</template>

<style scoped>
.history-panel { height: 430px; overflow: hidden; border: 1px solid var(--el-border-color-lighter); border-radius: 12px; background: var(--el-bg-color); }
.history-panel :deep(.el-table) { height: 100%; }
.history-panel :deep(.el-table__body-wrapper) { overflow: hidden; }
.history-pagination { display: flex; justify-content: flex-end; padding: 12px 0 18px; }
.trash-panel { height: 300px; }

.appearance-card { max-width: 760px; }
.appearance-row { display: flex; align-items: center; justify-content: space-between; gap: 20px; }
.appearance-title { font-weight: 600; margin-bottom: 4px; }


.section-title { font-weight: 600; }
.filter { width: 240px; margin-right: 8px; }
.cmd {
  font-size: 12px; font-family: Consolas, monospace;
  background: var(--el-fill-color); padding: 2px 6px; border-radius: 4px;
}
.key-row { display: flex; gap: 10px; margin-bottom: 10px; }
.key-input { max-width: 420px; }
.key-note { font-size: 12px; color: var(--el-text-color-secondary); line-height: 1.7; }
.mono :deep(textarea) { font-family: Consolas, monospace; }
.mt { margin-top: 10px; }
</style>
