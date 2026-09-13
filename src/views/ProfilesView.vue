<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Delete, MagicStick, Plus } from "@element-plus/icons-vue";
import { storeToRefs } from "pinia";
import { useAgentsStore } from "../stores/agents";
import {
  applyProfile,
  createProfile,
  deleteProfile,
  listLibrary,
  listMcpAll,
  listProfileItems,
  listProfiles,
} from "../api";
import type { LibraryItem, McpEntry, ProfileApplyResult, ProfileItem, ProfileMeta } from "../api/types";

const agentsStore = useAgentsStore();
const { agents } = storeToRefs(agentsStore);
const installedAgents = computed(() => agents.value.filter((a) => a.installed));

const profiles = ref<ProfileMeta[]>([]);
const itemsCache = ref<Record<number, ProfileItem[]>>({});
const library = ref<LibraryItem[]>([]);
const loading = ref(false);

const agentName = (id: string) => agents.value.find((a) => a.id === id)?.name ?? id;

async function refresh() {
  loading.value = true;
  try {
    profiles.value = await listProfiles();
    const next: Record<number, ProfileItem[]> = {};
    for (const p of profiles.value) {
      next[p.id] = await listProfileItems(p.id);
    }
    itemsCache.value = next;
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    loading.value = false;
  }
}

/* ---------- 新建 ---------- */

const createVisible = ref(false);
const creating = ref(false);
const form = reactive({ name: "", skills: [] as string[], mcps: [] as string[] });
const libraryItems = ref<LibraryItem[]>([]);
const mcpEntries = ref<McpEntry[]>([]);

async function openCreate() {
  Object.assign(form, { name: "", skills: [], mcps: [] });
  createVisible.value = true;
  try {
    const [lib, mcp] = await Promise.all([listLibrary(), listMcpAll()]);
    libraryItems.value = lib;
    mcpEntries.value = mcp.filter((e) => e.scope === "global");
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function submitCreate() {
  if (!form.name.trim()) {
    ElMessage.warning("请填写 Profile 名称");
    return;
  }
  if (!form.skills.length && !form.mcps.length) {
    ElMessage.warning("至少勾选一个 skill 或 MCP");
    return;
  }
  creating.value = true;
  try {
    const mcps = mcpEntries.value.filter((e) => form.mcps.includes(`${e.agentId}|${e.name}`));
    await createProfile(form.name.trim(), form.skills, mcps);
    ElMessage.success("Profile 已创建");
    createVisible.value = false;
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    creating.value = false;
  }
}

/* ---------- 应用 ---------- */

const applyVisible = ref(false);
const applying = ref(false);
const applyTarget = ref<ProfileMeta | null>(null);
const applyAgents = ref<string[]>([]);
const overwrite = ref(false);
const applyResults = ref<ProfileApplyResult[] | null>(null);

function openApply(p: ProfileMeta) {
  applyTarget.value = p;
  applyAgents.value = installedAgents.value.slice(0, 1).map((a) => a.id);
  overwrite.value = false;
  applyResults.value = null;
  applyVisible.value = true;
}

async function confirmApply() {
  if (!applyTarget.value) return;
  if (!applyAgents.value.length) {
    ElMessage.warning("请勾选目标 Agent");
    return;
  }
  applying.value = true;
  try {
    const results = await applyProfile(applyTarget.value.id, applyAgents.value, overwrite.value);
    applyResults.value = results;
    const ok = results.filter((r) => r.ok).length;
    if (ok) ElMessage.success(`${ok} 项已应用`);
    results.filter((r) => !r.ok).forEach((r) => ElMessage.error(`${r.name} → ${agentName(r.agentId)}: ${r.error}`));
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    applying.value = false;
  }
}

async function onDelete(p: ProfileMeta) {
  try {
    await ElMessageBox.confirm(`删除 Profile「${p.name}」？不影响已下发的配置。`, "确认删除", {
      type: "warning",
    });
  } catch {
    return;
  }
  try {
    await deleteProfile(p.id);
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

onMounted(async () => {
  await refresh();
  library.value = await listLibrary().catch(() => []);
});
</script>

<template>
  <div>
    <div class="toolbar">
      <el-button type="primary" :icon="Plus" @click="openCreate">新建 Profile</el-button>
      <span class="hint">把一组 skill（中央库）+ MCP（全局条目）保存为命名组合，一键应用到指定 Agent（写入前自动快照）</span>
    </div>

    <el-table :data="profiles" v-loading="loading" stripe>
      <el-table-column prop="name" label="Profile" min-width="180" />
      <el-table-column label="包含内容" min-width="360">
        <template #default="{ row }">
          <template v-if="(itemsCache[row.id] || []).length">
            <el-tag
              v-for="it in itemsCache[row.id]"
              :key="it.id"
              size="small"
              :type="it.kind === 'skill' ? 'success' : 'warning'"
              effect="plain"
              class="agent-tag"
            >{{ it.kind === "skill" ? "S" : "M" }}·{{ it.refName }}</el-tag>
          </template>
          <span v-else class="hint">（空）</span>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="180" fixed="right">
        <template #default="{ row }">
          <el-button size="small" type="primary" :icon="MagicStick" @click="openApply(row)">应用到…</el-button>
          <el-button size="small" type="danger" plain :icon="Delete" @click="onDelete(row)" />
        </template>
      </el-table-column>
      <template #empty>还没有 Profile——点「新建 Profile」把常用组合保存下来</template>
    </el-table>

    <!-- 新建对话框 -->
    <el-dialog v-model="createVisible" title="新建 Profile" width="560px">
      <el-form label-position="top">
        <el-form-item label="名称" required>
          <el-input v-model="form.name" placeholder="如：工作 / 写作 / 前端项目" />
        </el-form-item>
        <el-form-item :label="`Skill（来自中央库，${libraryItems.length} 个可选）`">
          <el-checkbox-group v-model="form.skills">
            <el-checkbox v-for="s in libraryItems" :key="s.name" :value="s.name" :label="s.name" />
          </el-checkbox-group>
          <div v-if="!libraryItems.length" class="hint">中央库为空——先去市场安装或在 Skill 中心收编</div>
        </el-form-item>
        <el-form-item :label="`MCP（全局条目，${mcpEntries.length} 个可选）`">
          <el-checkbox-group v-model="form.mcps">
            <el-checkbox
              v-for="e in mcpEntries"
              :key="`${e.agentId}|${e.name}`"
              :value="`${e.agentId}|${e.name}`"
              :label="`${e.name}（${e.command ?? e.url ?? ''}）`"
            />
          </el-checkbox-group>
          <div v-if="!mcpEntries.length" class="hint">没有读到全局 MCP 条目</div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createVisible = false">取消</el-button>
        <el-button type="primary" :loading="creating" @click="submitCreate">创建</el-button>
      </template>
    </el-dialog>

    <!-- 应用对话框 -->
    <el-dialog v-model="applyVisible" :title="`应用 Profile：${applyTarget?.name ?? ''}`" width="480px">
      <el-form label-position="top">
        <el-form-item label="应用到" required>
          <el-checkbox-group v-model="applyAgents">
            <el-checkbox v-for="a in installedAgents" :key="a.id" :value="a.id" :label="a.name" />
          </el-checkbox-group>
        </el-form-item>
        <el-form-item>
          <el-switch v-model="overwrite" active-text="同名条目直接覆盖" />
        </el-form-item>
      </el-form>
      <template v-if="applyResults">
        <el-divider />
        <div v-for="(r, i) in applyResults" :key="i" class="deploy-line">
          <el-tag size="small" :type="r.ok ? 'success' : 'danger'">{{ r.kind === "skill" ? "S" : "M" }}·{{ r.name }} → {{ agentName(r.agentId) }}</el-tag>
          <span class="hint">{{ r.error ?? "完成" }}</span>
        </div>
      </template>
      <template #footer>
        <el-button @click="applyVisible = false">关闭</el-button>
        <el-button type="primary" :loading="applying" @click="confirmApply">执行应用</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.toolbar { display: flex; align-items: center; gap: 12px; margin-bottom: 14px; }
.hint { font-size: 12px; color: var(--el-text-color-secondary); }
.agent-tag { margin-right: 6px; margin-bottom: 2px; }
.deploy-line { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
</style>
