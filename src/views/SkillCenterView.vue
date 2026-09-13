<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Collection, Delete, Refresh, Search, View } from "@element-plus/icons-vue";
import {
  adoptSkill,
  listLibrary,
  listSkills,
  readLibrarySkill,
  readSkillMd,
  removeSkill,
} from "../api";
import type { AdoptReport, LibraryItem, SkillEntry } from "../api/types";
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
          <el-table-column label="操作" width="230" fixed="right">
            <template #default="{ row }">
              <el-button size="small" :icon="View" @click="openAgentSkillDetail(row)">详情</el-button>
              <el-button size="small" type="primary" plain :icon="Collection" @click="startAdopt(row)">收编</el-button>
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
</style>
