<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Collection, Delete, FolderAdd, Refresh, Search, SwitchButton, View } from "@element-plus/icons-vue";
import {
  adoptSkill,
  applySkillSync,
  importSkillFolder,
  listLibrary,
  listSkills,
  planSkillSync,
  readLibrarySkill,
  readSkillMd,
  removeSkill,
} from "../api";
import type { AdoptReport, LibraryItem, PropagatePlan, SkillEntry, SyncReport } from "../api/types";
import { renderMarkdown } from "../utils/markdown";

const activeTab = ref("agents");
const skills = ref<SkillEntry[]>([]);
const library = ref<LibraryItem[]>([]);
const loading = ref(false);
const keyword = ref("");
const agentFilter = ref("");
const scopeFilter = ref("");

const agentOptions = computed(() => [...new Set(skills.value.map((s) => s.agentId))]);

const filteredSkills = computed(() =>
  skills.value.filter((s) => {
    const okAgent = !agentFilter.value || s.agentId === agentFilter.value;
    const okScope =
      !scopeFilter.value ||
      (scopeFilter.value === "user" ? s.scope === "user" : s.scope.startsWith("project:"));
    const kw = keyword.value.trim().toLowerCase();
    const okKw =
      !kw ||
      s.name.toLowerCase().includes(kw) ||
      (s.description ?? "").toLowerCase().includes(kw);
    return okAgent && okScope && okKw;
  })
);

const scopeTag = (scope: string) => (scope === "user" ? "success" : "warning");
const scopeText = (scope: string) => (scope === "user" ? "用户级" : "项目级");

async function refresh() {
  loading.value = true;
  try {
    const [s, lib] = await Promise.all([listSkills(), listLibrary()]);
    skills.value = s;
    library.value = lib;
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    loading.value = false;
  }
}

/* ---------- 详情抽屉 ---------- */

const detailVisible = ref(false);
const detailTitle = ref("");
const detailMeta = ref<string[]>([]);
const detailMd = ref("");

const renderedMd = computed(() => renderMarkdown(detailMd.value));

async function openAgentSkillDetail(s: SkillEntry) {
  detailTitle.value = s.name;
  detailMeta.value = [s.dir, s.description ?? "（无描述）"];
  detailMd.value = "";
  detailVisible.value = true;
  try {
    detailMd.value = await readSkillMd(s.dir);
  } catch (e) {
    detailMd.value = `读取失败：${e}`;
  }
}

async function openLibraryDetail(item: LibraryItem) {
  detailTitle.value = item.name;
  detailMeta.value = [
    `来源：${item.sourceAgent ?? "—"}`,
    `原路径：${item.sourcePath ?? "—"}`,
    `文件数：${item.fileCount}`,
  ];
  detailMd.value = "";
  detailVisible.value = true;
  try {
    detailMd.value = await readLibrarySkill(item.name);
  } catch (e) {
    detailMd.value = `读取失败：${e}`;
  }
}

/* ---------- 收编 ---------- */

const adoptDialogVisible = ref(false);
const adoptPlan = ref<AdoptReport | null>(null);
const adoptTarget = ref<SkillEntry | null>(null);

async function startAdopt(s: SkillEntry) {
  try {
    adoptPlan.value = await adoptSkill(s.agentId, s.name, s.scope, true);
  } catch (e) {
    ElMessage.error(String(e));
    return;
  }
  adoptTarget.value = s;
  adoptDialogVisible.value = true;
}

async function confirmAdopt() {
  const s = adoptTarget.value;
  if (!s) return;
  try {
    const r = await adoptSkill(s.agentId, s.name, s.scope, false);
    if (r.conflict) {
      ElMessage.warning(`中央库已有同名条目「${r.skillName}」，已拒绝覆盖`);
    } else {
      ElMessage.success(`已收编 ${r.files.length} 个文件到中央库`);
    }
    adoptDialogVisible.value = false;
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

/* ---------- 删除（回收站式，可恢复，见设置页） ---------- */

async function onDelete(s: SkillEntry) {
  try {
    await ElMessageBox.confirm(
      `把 ${s.agentId} 的「${s.name}」移入 AgentHub 回收站？\n原目录：${s.dir}\n\n不会立即销毁，可在设置页随时恢复。`,
      "确认删除",
      { type: "warning", confirmButtonText: "移入回收站", cancelButtonText: "取消" }
    );
  } catch {
    return;
  }
  try {
    await removeSkill(s.agentId, s.name, s.scope);
    ElMessage.success("已移入回收站（设置页可恢复）");
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

/* ---------- 文件夹导入 ---------- */

const importDialogVisible = ref(false);
const importPath = ref("");
const importPlan = ref<AdoptReport | null>(null);
const importing = ref(false);

function openImport() {
  importPath.value = "";
  importPlan.value = null;
  importDialogVisible.value = true;
}

async function previewImport() {
  const p = importPath.value.trim().replace(/^"|"$/g, "");
  if (!p) {
    ElMessage.warning("请输入 skill 所在文件夹路径");
    return;
  }
  try {
    importPlan.value = await importSkillFolder(p, true);
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function confirmImport() {
  const p = importPath.value.trim().replace(/^"|"$/g, "");
  importing.value = true;
  try {
    const r = await importSkillFolder(p, false);
    if (r.conflict) {
      ElMessage.warning(`中央库已有同名条目「${r.skillName}」，已拒绝覆盖`);
    } else {
      ElMessage.success(`已导入 ${r.files.length} 个文件到中央库`);
      importDialogVisible.value = false;
      await refresh();
    }
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    importing.value = false;
  }
}

/* ---------- 跨 Agent 同步 ---------- */

const syncVisible = ref(false);
const syncSource = ref<SkillEntry | null>(null);
const syncTargets = ref<string[]>([]);
const syncPlans = ref<Record<string, PropagatePlan | null>>({});
const syncReports = ref<Record<string, SyncReport | null>>({});
const syncDelete = ref(false);
const syncing = ref(false);
const planning = ref(false);

const otherAgents = computed(() =>
  skills.value
    .map((s) => s.agentId)
    .filter((id, i, arr) => arr.indexOf(id) === i && (!syncSource.value || id !== syncSource.value.agentId))
);

async function openSync(s: SkillEntry) {
  syncSource.value = s;
  syncTargets.value = [];
  syncPlans.value = {};
  syncReports.value = {};
  syncDelete.value = false;
  syncVisible.value = true;
}

async function buildPlans() {
  const s = syncSource.value;
  if (!s || !syncTargets.value.length) {
    ElMessage.warning("请先勾选目标 Agent，再生成差异预览");
    return;
  }
  planning.value = true;
  try {
    const next: Record<string, PropagatePlan | null> = {};
    for (const t of syncTargets.value) {
      next[t] = await planSkillSync(s.agentId, s.name, s.scope, t);
    }
    syncPlans.value = next;
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    planning.value = false;
  }
}

async function confirmSync() {
  const s = syncSource.value;
  if (!s || !syncTargets.value.length) return;
  syncing.value = true;
  try {
    const reports: Record<string, SyncReport | null> = {};
    for (const t of syncTargets.value) {
      const plan = syncPlans.value[t];
      if (!plan || plan.identical) {
        reports[t] = null;
        continue;
      }
      reports[t] = await applySkillSync(s.agentId, s.name, s.scope, plan, syncDelete.value);
    }
    syncReports.value = reports;
    ElMessage.success("同步完成（删除差异按你的确认处理）");
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    syncing.value = false;
  }
}

onMounted(refresh);
</script>

<template>
  <div>
    <el-tabs v-model="activeTab">
      <el-tab-pane label="各 Agent 的 Skill" name="agents">
        <div class="toolbar">
          <el-input v-model="keyword" :prefix-icon="Search" placeholder="搜索名称 / 描述" clearable class="kw" />
          <el-select v-model="agentFilter" placeholder="全部 Agent" clearable class="sel">
            <el-option v-for="a in agentOptions" :key="a" :label="a" :value="a" />
          </el-select>
          <el-select v-model="scopeFilter" placeholder="全部作用域" clearable class="sel">
            <el-option label="用户级" value="user" />
            <el-option label="项目级" value="project" />
          </el-select>
          <el-button :icon="FolderAdd" @click="openImport">导入文件夹</el-button>
          <el-button :icon="Refresh" :loading="loading" @click="refresh">刷新</el-button>
        </div>

        <el-table :data="filteredSkills" v-loading="loading" stripe>
          <el-table-column label="Skill" min-width="160">
            <template #default="{ row }">
              <span class="skill-name">{{ row.name }}</span>
              <el-tag v-if="!row.hasSkillMd" size="small" type="danger" effect="plain" class="ml">缺 SKILL.md</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="agentId" label="Agent" width="130" />
          <el-table-column label="作用域" width="100">
            <template #default="{ row }">
              <el-tag size="small" :type="scopeTag(row.scope)" effect="plain">{{ scopeText(row.scope) }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="描述" min-width="260">
            <template #default="{ row }">{{ row.description ?? "—" }}</template>
          </el-table-column>
          <el-table-column label="操作" width="290" fixed="right">
            <template #default="{ row }">
              <el-button size="small" :icon="View" @click="openAgentSkillDetail(row)">详情</el-button>
              <el-button size="small" type="primary" plain :icon="Collection" @click="startAdopt(row)">收编</el-button>
              <el-button size="small" :icon="SwitchButton" @click="openSync(row)">同步…</el-button>
              <el-button size="small" type="danger" plain :icon="Delete" @click="onDelete(row)" />
            </template>
          </el-table-column>
          <template #empty>没有匹配的 skill</template>
        </el-table>
      </el-tab-pane>

      <el-tab-pane :label="`中央库（${library.length}）`" name="library">
        <el-alert
          title="中央库是收编后的单一事实来源；P1 将在此基础上支持「一键安装到任意 Agent」"
          type="info"
          :closable="false"
          class="mb"
        />
        <el-table :data="library" v-loading="loading" stripe>
          <el-table-column prop="name" label="Skill" min-width="160" />
          <el-table-column label="来源 Agent" width="130">
            <template #default="{ row }">{{ row.sourceAgent ?? "—" }}</template>
          </el-table-column>
          <el-table-column label="描述" min-width="240">
            <template #default="{ row }">{{ row.description ?? "—" }}</template>
          </el-table-column>
          <el-table-column prop="fileCount" label="文件数" width="90" />
          <el-table-column label="操作" width="100" fixed="right">
            <template #default="{ row }">
              <el-button size="small" :icon="View" @click="openLibraryDetail(row)">详情</el-button>
            </template>
          </el-table-column>
          <template #empty>还没有收编任何 skill——去「各 Agent 的 Skill」里点「收编」</template>
        </el-table>
      </el-tab-pane>
    </el-tabs>

    <el-drawer v-model="detailVisible" :title="detailTitle" size="560px">
      <div class="meta">
        <div v-for="m in detailMeta" :key="m" class="meta-line">{{ m }}</div>
      </div>
      <div v-if="renderedMd" class="md-body" v-html="renderedMd"></div>
      <div v-else class="md-empty">（无 SKILL.md 内容）</div>
    </el-drawer>

    <!-- 同步对话框 -->
    <el-dialog v-model="syncVisible" :title="`同步：${syncSource?.name ?? ''}（源：${syncSource?.agentId ?? ''}）`" width="560px">
      <el-form label-position="top">
        <el-form-item label="目标 Agent" required>
          <el-checkbox-group v-model="syncTargets" @change="syncPlans = {}; syncReports = {}">
            <el-checkbox v-for="a in otherAgents" :key="a" :value="a" :label="a" />
          </el-checkbox-group>
          <div v-if="!otherAgents.length" class="hint">只有一个 Agent 有 skill，没有可同步的目标</div>
        </el-form-item>
        <el-button size="small" :loading="planning" @click="buildPlans">生成差异预览</el-button>

        <div v-for="(plan, t) in syncPlans" :key="t" class="plan-block">
          <template v-if="plan">
            <div class="plan-head">
              <b>{{ t }}</b>
              <el-tag v-if="plan.identical" size="small" type="info">完全一致，无需同步</el-tag>
              <el-tag v-else-if="plan.targetAbsent" size="small" type="success">目标没有，将整体安装（{{ plan.copy.length }} 个文件）</el-tag>
              <template v-else>
                <el-tag size="small" type="warning">更新 {{ plan.copy.length }} 个</el-tag>
                <el-tag size="small" type="danger" effect="plain">目标多出 {{ plan.deletions.length }} 个</el-tag>
              </template>
            </div>
            <div v-if="plan.copy.length" class="plan-files">复制：{{ plan.copy.join("、") }}</div>
            <div v-if="plan.deletions.length" class="plan-files del">
              目标多出：{{ plan.deletions.join("、") }}
              <el-checkbox v-model="syncDelete" class="del-check">确认删除这些文件</el-checkbox>
            </div>
          </template>
          <div v-else-if="syncReports[t] === null" class="plan-head">{{ t }}：无需变动</div>
          <template v-else-if="syncReports[t]">
            <div class="plan-head">{{ t }}：已复制 {{ syncReports[t]!.copied }} 个，删除 {{ syncReports[t]!.deleted }} 个</div>
            <div v-if="syncReports[t]!.heldBackDeletions.length" class="plan-files del">
              扣留未删：{{ syncReports[t]!.heldBackDeletions.join("、") }}
            </div>
            <div class="plan-files">快照：{{ syncReports[t]!.backupDir }}</div>
          </template>
        </div>
      </el-form>
      <template #footer>
        <el-button @click="syncVisible = false">关闭</el-button>
        <el-button type="primary" :loading="syncing" :disabled="!syncTargets.length || !Object.keys(syncPlans).length" @click="confirmSync">
          执行同步
        </el-button>
      </template>
    </el-dialog>

    <!-- 文件夹导入对话框 -->
    <el-dialog v-model="importDialogVisible" title="从文件夹导入 skill" width="480px">
      <el-input v-model="importPath" placeholder="skill 文件夹完整路径（含 SKILL.md）" clearable @keyup.enter="previewImport">
        <template #append>
          <el-button @click="previewImport">预览</el-button>
        </template>
      </el-input>
      <template v-if="importPlan">
        <el-alert
          v-if="importPlan.conflict"
          title="中央库已存在同名条目，执行将被拒绝（不会覆盖）"
          type="warning"
          :closable="false"
          class="mt"
        />
        <p class="mt">共 {{ importPlan.files.length }} 个文件 → {{ importPlan.targetDir }}</p>
      </template>
      <template #footer>
        <el-button @click="importDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="importing" :disabled="!importPlan || importPlan.conflict" @click="confirmImport">导入</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="adoptDialogVisible" title="收编预览（dry-run）" width="480px">
      <template v-if="adoptPlan">
        <el-alert
          v-if="adoptPlan.conflict"
          title="中央库已存在同名条目，执行将被拒绝（不会覆盖）"
          type="warning"
          :closable="false"
          class="mb"
        />
        <p class="mb">
          将把 <b>{{ adoptTarget?.agentId }}</b> 的 <b>{{ adoptPlan.skillName }}</b>
          （{{ adoptPlan.files.length }} 个文件）收编到：
        </p>
        <p class="path mb">{{ adoptPlan.targetDir }}</p>
        <el-input type="textarea" :rows="6" :model-value="adoptPlan.files.join('\n')" readonly class="mono" />
      </template>
      <template #footer>
        <el-button @click="adoptDialogVisible = false">取消</el-button>
        <el-button type="primary" :disabled="adoptPlan?.conflict" @click="confirmAdopt">执行收编</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.toolbar { display: flex; gap: 10px; margin-bottom: 14px; }
.kw { width: 260px; }
.sel { width: 160px; }
.mb { margin-bottom: 12px; }
.ml { margin-left: 6px; }
.skill-name { font-weight: 600; }
.meta { margin-bottom: 12px; }
.meta-line {
  font-size: 12px; color: var(--el-text-color-secondary);
  font-family: Consolas, monospace; word-break: break-all;
  margin-bottom: 4px;
}
.md-body {
  font-size: 13px; line-height: 1.7;
  max-height: 70vh; overflow: auto;
}
.md-body :deep(h1), .md-body :deep(h2), .md-body :deep(h3) { margin: 14px 0 8px; }
.md-body :deep(p) { margin: 8px 0; }
.md-body :deep(ul), .md-body :deep(ol) { padding-left: 20px; margin: 8px 0; }
.md-body :deep(code) {
  font-family: Consolas, monospace; font-size: 12px;
  background: var(--el-fill-color); padding: 1px 5px; border-radius: 4px;
}
.md-body :deep(pre) {
  background: var(--el-fill-color-light); border-radius: 6px;
  padding: 10px; overflow: auto;
}
.md-body :deep(pre code) { background: none; padding: 0; }
.md-body :deep(blockquote) {
  border-left: 3px solid var(--el-border-color); color: var(--el-text-color-secondary);
  padding-left: 10px; margin: 8px 0;
}
.md-body :deep(table) { border-collapse: collapse; margin: 8px 0; }
.md-body :deep(th), .md-body :deep(td) { border: 1px solid var(--el-border-color-lighter); padding: 4px 8px; }
.md-empty {
  font-size: 12px; color: var(--el-text-color-secondary);
  background: var(--el-fill-color-light); border-radius: 6px; padding: 12px;
}
.path { font-family: Consolas, monospace; font-size: 12px; word-break: break-all; }
.mono :deep(textarea) { font-family: Consolas, monospace; }
.mt { margin-top: 12px; }
.plan-block { border: 1px solid var(--el-border-color-lighter); border-radius: 6px; padding: 10px; margin-bottom: 10px; }
.plan-head { margin-bottom: 6px; display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
.plan-files { font-size: 12px; color: var(--el-text-color-secondary); font-family: Consolas, monospace; word-break: break-all; }
.plan-files.del { color: var(--el-color-danger); }
.del-check { margin-left: 10px; }
.hint { font-size: 12px; color: var(--el-text-color-secondary); }
</style>
