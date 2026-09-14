<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
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

/* ---------- 自定义 Agent ---------- */

const customAgentsRaw = ref("[]");
const customAgentsSaving = ref(false);

interface CustomAgentForm {
  id: string;
  name: string;
  kind: AgentDescriptor["kind"];
  mcpFormat: string;
  reload: string;
  mcpConfigPath: string;
  skillDir: string;
}

const customAgentForms = ref<CustomAgentForm[]>([]);
const customAgentAdvanced = ref(false);
const newAgent = reactive<CustomAgentForm>({
  id: "", name: "", kind: "cli", mcpFormat: "json-map", reload: "restart", mcpConfigPath: "", skillDir: "",
});

function descriptorToForm(agent: AgentDescriptor): CustomAgentForm {
  return {
    id: agent.id, name: agent.name, kind: agent.kind, mcpFormat: agent.mcpFormat, reload: agent.reload,
    mcpConfigPath: agent.mcpConfigPaths.windows?.[0] ?? agent.mcpConfigPaths.macos?.[0] ?? agent.mcpConfigPaths.linux?.[0] ?? "",
    skillDir: agent.skillDirs.windows?.[0] ?? agent.skillDirs.macos?.[0] ?? agent.skillDirs.linux?.[0] ?? "",
  };
}

function addCustomAgent() {
  if (!newAgent.id.trim() || !newAgent.name.trim()) {
    ElMessage.warning("请填写 Agent 名称和 ID");
    return;
  }
  if (customAgentForms.value.some((a) => a.id === newAgent.id.trim())) {
    ElMessage.warning("Agent ID 已存在，请换一个");
    return;
  }
  customAgentForms.value.push({ ...newAgent, id: newAgent.id.trim(), name: newAgent.name.trim() });
  Object.assign(newAgent, { id: "", name: "", kind: "cli", mcpFormat: "json-map", reload: "restart", mcpConfigPath: "", skillDir: "" });
}

function removeCustomAgent(index: number) { customAgentForms.value.splice(index, 1); }

function formToDescriptor(agent: CustomAgentForm): AgentDescriptor {
  const path = agent.mcpConfigPath.trim() ? [agent.mcpConfigPath.trim()] : [];
  const skill = agent.skillDir.trim() ? [agent.skillDir.trim()] : [];
  return {
    id: agent.id.trim(), name: agent.name.trim(), kind: agent.kind,
    mcpConfigPaths: { windows: path, macos: path, linux: path },
    skillDirs: { windows: skill, macos: skill, linux: skill },
    mcpFormat: agent.mcpFormat, reload: agent.reload,
  };
}

async function refreshCustomAgents() {
  try {
    customAgentsRaw.value = await getCustomAgents();
    const parsed = JSON.parse(customAgentsRaw.value) as AgentDescriptor[];
    customAgentForms.value = Array.isArray(parsed) ? parsed.map(descriptorToForm) : [];
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function saveCustomAgents() {
  customAgentsSaving.value = true;
  try {
    const agents = customAgentForms.value.map(formToDescriptor);
    if (agents.some((a) => !a.id || !a.name || !a.mcpConfigPaths.windows.length)) {
      throw new Error("每个 Agent 至少需要填写名称、ID 和 MCP 配置文件路径");
    }
    const raw = JSON.stringify(agents, null, 2);
    await setCustomAgents(raw);
    customAgentsRaw.value = raw;
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
  <div class="settings-page">
    <!-- 外观 -->
    <div class="settings-group">
      <div class="settings-group-header">外观</div>
      <div class="settings-group-body">
        <div class="settings-row">
          <div class="settings-row-label">
            <div class="settings-row-title">主题</div>
            <div class="settings-row-desc">跟随系统外观，或固定使用日间 / 夜间模式</div>
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
      </div>
    </div>

    <!-- 数据目录 -->
    <div class="settings-group">
      <div class="settings-group-header">数据目录</div>
      <div class="settings-group-body">
        <div class="settings-row">
          <div class="settings-row-label">
            <div class="settings-row-title">文件位置</div>
            <div class="settings-row-desc"><code class="cmd">%APPDATA%\AgentHub</code> —— 快照、中央库、SQLite 都在这里</div>
          </div>
        </div>
        <div class="settings-row">
          <div class="settings-row-label">
            <div class="settings-row-title">快照策略</div>
            <div class="settings-row-desc">任何写配置动作前自动快照；回滚前也会对当前文件再快照一次（可撤销）</div>
          </div>
        </div>
      </div>
    </div>

    <!-- SkillsMP API 密钥 -->
    <div class="settings-group">
      <div class="settings-group-header">SkillsMP API 密钥</div>
      <div class="settings-group-body">
        <div class="settings-row settings-row--col">
          <div class="settings-row-label">
            <div class="settings-row-title">API 密钥</div>
            <div class="settings-row-desc">
              <template v-if="keyStatus.set">
                已设置：<code class="cmd">{{ keyStatus.masked }}</code>
              </template>
              <template v-else>未设置。匿名可用（每天 50 次搜索）；配置密钥后每天 500 次并启用按技能功能（语义）排序的搜索。</template>
            </div>
          </div>
          <div class="settings-row-controls">
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
          </div>
        </div>
      </div>
    </div>

    <!-- 自定义 Agent -->
    <div class="settings-group">
      <div class="settings-group-header">自定义 Agent</div>
      <div class="settings-group-body">
        <div class="settings-row settings-row--col">
          <div class="settings-row-label">
            <div class="settings-row-title">添加 Agent</div>
            <div class="settings-row-desc">不用编写 JSON，只需填写 Agent 名称和配置文件位置即可。保存后重启 AgentHub 生效。</div>
          </div>
          <div class="settings-row-controls">
            <div class="agent-form-grid">
              <el-input v-model="newAgent.name" placeholder="显示名称，如 DeepSeek Harness" />
              <el-input v-model="newAgent.id" placeholder="唯一 ID，如 dsh" />
              <el-select v-model="newAgent.kind" placeholder="类型">
                <el-option label="命令行工具" value="cli" />
                <el-option label="IDE 插件" value="ide" />
                <el-option label="桌面应用" value="desktop" />
              </el-select>
              <el-input v-model="newAgent.mcpConfigPath" placeholder="MCP 配置文件路径，如 ~/.dsh/dsh-mcp.json" />
              <el-input v-model="newAgent.skillDir" placeholder="Skill 目录（可选）" />
              <el-button type="primary" @click="addCustomAgent">添加 Agent</el-button>
            </div>

            <el-empty v-if="!customAgentForms.length" description="还没有自定义 Agent" :image-size="48" />
            <div v-for="(agent, index) in customAgentForms" :key="agent.id" class="custom-agent-item">
              <div class="agent-item-title"><b>{{ agent.name }}</b><el-tag size="small" effect="plain">{{ agent.id }}</el-tag></div>
              <div class="agent-item-path">MCP：{{ agent.mcpConfigPath || "未填写" }}<span v-if="agent.skillDir"> · Skill：{{ agent.skillDir }}</span></div>
              <el-button text type="danger" @click="removeCustomAgent(index)">移除</el-button>
            </div>
            <div class="custom-agent-actions">
              <el-button type="primary" :loading="customAgentsSaving" @click="saveCustomAgents">保存配置</el-button>
              <el-button text @click="customAgentAdvanced = !customAgentAdvanced">{{ customAgentAdvanced ? "收起高级 JSON" : "高级 JSON" }}</el-button>
            </div>
            <el-collapse-transition>
              <div v-if="customAgentAdvanced" class="advanced-json">
                <div class="settings-row-desc mb">高级模式仅用于导入或微调完整配置；普通用户无需使用。</div>
                <el-input v-model="customAgentsRaw" type="textarea" :rows="10" spellcheck="false" class="mono" />
                <el-button class="mt" @click="customAgentsRaw = JSON.stringify(customAgentForms.map(formToDescriptor), null, 2)">从当前表单生成 JSON</el-button>
              </div>
            </el-collapse-transition>
          </div>
        </div>
      </div>
    </div>

    <!-- Agent 路径覆写 -->
    <div class="settings-group">
      <div class="settings-group-header">Agent 路径覆写（进阶）</div>
      <div class="settings-group-body">
        <div class="settings-row settings-row--col">
          <div class="settings-row-label">
            <div class="settings-row-title">覆写配置</div>
            <div class="settings-row-desc">
              当 Agent 装在自定义位置时，可覆写其配置路径与 skill 目录（仅当前操作系统生效，路径支持 ~ 前缀）。
              示例：{"claude-code": {"mcpConfigPaths": ["D:/portable/.claude.json"]}}。
              保存后<b>重启应用</b>生效；留空对象 {} 恢复默认。
            </div>
          </div>
          <div class="settings-row-controls">
            <el-input
              v-model="overridesRaw"
              type="textarea"
              :rows="4"
              spellcheck="false"
              class="mono"
              placeholder='{}'
            />
            <el-button type="primary" class="mt" :loading="overridesSaving" @click="saveOverrides">保存覆写</el-button>
          </div>
        </div>
      </div>
    </div>

    <!-- 快照历史 -->
    <div class="settings-group">
      <div class="settings-group-header">快照历史（{{ snapshots.length }}）</div>
      <div class="settings-group-body">
        <div class="snapshot-toolbar">
          <el-input v-model="filter" placeholder="按原路径过滤" clearable size="small" class="filter" />
          <el-button size="small" :icon="Refresh" :loading="loading" @click="refresh">刷新</el-button>
          <el-button size="small" type="danger" plain @click="onPrune">清理旧快照（保留 50）</el-button>
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
          class="table-pagination"
        />
      </div>
    </div>

    <!-- 回收站 -->
    <div class="settings-group">
      <div class="settings-group-header">回收站（{{ trashItems.length }}）</div>
      <div class="settings-group-body">
        <div class="snapshot-toolbar">
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
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  max-width: 860px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

/* macOS 风格分组 */
.settings-group {
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 14px;
  overflow: hidden;
}
.settings-group-header {
  padding: 20px 24px 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: .02em;
}
.settings-group-body {
  padding: 14px 0 0;
}

/* 设置行：左侧标题+描述，右侧控件 */
.settings-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 24px;
  gap: 20px;
  min-height: 48px;
}
.settings-row + .settings-row {
  border-top: 1px solid var(--el-border-color-lighter);
}
.settings-row--col {
  flex-direction: column;
  align-items: stretch;
}
.settings-row-label { flex: 1; min-width: 0; }
.settings-row-title { font-size: 14px; font-weight: 500; color: var(--el-text-color-primary); margin-bottom: 2px; }
.settings-row-desc { font-size: 12px; color: var(--el-text-color-secondary); line-height: 1.6; }
.settings-row-controls { flex: 0 0 auto; width: 100%; max-width: 520px; margin-top: 6px; }
.settings-row--col .settings-row-controls { max-width: 100%; }

/* 通用 */
.mb { margin-bottom: 10px; }
.mt { margin-top: 10px; }
.cmd {
  font-size: 12px; font-family: Consolas, monospace;
  background: var(--el-fill-color); padding: 1px 6px; border-radius: 4px;
  word-break: break-all;
}
.key-row { display: flex; gap: 10px; align-items: center; }
.key-input { flex: 1; }
.mono :deep(textarea) { font-family: Consolas, monospace; }

/* 自定义 Agent */
.agent-form-grid { display: grid; grid-template-columns: 1fr 180px 150px; gap: 10px; }
.agent-form-grid > :nth-child(4), .agent-form-grid > :nth-child(5) { grid-column: span 2; }
.custom-agent-item { display: flex; align-items: center; gap: 10px; padding: 12px 0; border-top: 1px solid var(--el-border-color-lighter); }
.agent-item-title { display: flex; align-items: center; gap: 8px; min-width: 200px; }
.agent-item-path { flex: 1; color: var(--el-text-color-secondary); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.custom-agent-actions { display: flex; align-items: center; gap: 6px; margin-top: 12px; }
.advanced-json { border-top: 1px solid var(--el-border-color-lighter); margin-top: 12px; padding-top: 14px; }

/* 快照工具栏 */
.snapshot-toolbar { display: flex; gap: 8px; align-items: center; margin-bottom: 12px; }
.filter { width: 240px; }

/* 快照列表 */
.history-panel { height: 430px; overflow: hidden; border: 1px solid var(--el-border-color-lighter); border-radius: 12px; background: var(--el-bg-color); }
.history-panel :deep(.el-table) { height: 100%; }
.history-panel :deep(.el-table__body-wrapper) { overflow: hidden; }
.trash-panel { height: 300px; }
.table-pagination { display: flex; justify-content: flex-end; padding: 12px 0 4px; }
</style>
