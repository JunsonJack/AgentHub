<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { ElMessage } from "element-plus";
import { Delete, Download, Edit, FolderAdd, Link, Refresh, Search, View } from "@element-plus/icons-vue";
import { useAgentsStore } from "../stores/agents";
import { storeToRefs } from "pinia";
import {
  addCollectionItem,
  deployLibrarySkill,
  deleteCollectionItem,
  listCollection,
  listCurated,
  listLibrary,
  marketInstallGit,
  marketInstallSkillsSh,
  marketPreview,
  marketSearch,
  updateCollectionItem,
} from "../api";
import type { AdoptReport, CollectionEntry, DeployResult, LibraryItem, MarketPreview, MarketSkill } from "../api/types";

const agentsStore = useAgentsStore();
const { agents } = storeToRefs(agentsStore);
const installedAgents = computed(() => agents.value.filter((a) => a.installed));

const activeTab = ref("market");

/* ---------- 收藏集 ---------- */

const curated = ref<CollectionEntry[]>([]);
const myItems = ref<CollectionEntry[]>([]);
const collectionLoading = ref(false);
const itemDialogVisible = ref(false);
const itemSaving = ref(false);
const itemForm = reactive({
  id: null as number | null,
  kind: "skill",
  name: "",
  source: "",
  tags: "",
  note: "",
  stars: 0,
});

async function refreshCollection() {
  collectionLoading.value = true;
  try {
    const [c, u] = await Promise.all([listCurated(), listCollection()]);
    curated.value = c;
    myItems.value = u;
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    collectionLoading.value = false;
  }
}

function openNewItem() {
  Object.assign(itemForm, { id: null, kind: "skill", name: "", source: "", tags: "", note: "", stars: 0 });
  itemDialogVisible.value = true;
}

function openEditItem(entry: CollectionEntry) {
  Object.assign(itemForm, {
    id: entry.id,
    kind: entry.kind,
    name: entry.name,
    source: entry.source,
    tags: entry.tags.join(", "),
    note: entry.note,
    stars: entry.stars,
  });
  itemDialogVisible.value = true;
}

async function saveItem() {
  if (!itemForm.name.trim() || !itemForm.source.trim()) {
    ElMessage.warning("名称与来源必填");
    return;
  }
  itemSaving.value = true;
  try {
    const entry: CollectionEntry = {
      id: itemForm.id,
      kind: itemForm.kind,
      name: itemForm.name.trim(),
      source: itemForm.source.trim(),
      tags: itemForm.tags.split(/[,，]/).map((t) => t.trim()).filter(Boolean),
      note: itemForm.note.trim(),
      stars: Number(itemForm.stars) || 0,
      builtIn: false,
    };
    if (entry.id == null) {
      await addCollectionItem(entry);
      ElMessage.success("已添加");
    } else {
      await updateCollectionItem(entry);
      ElMessage.success("已更新");
    }
    itemDialogVisible.value = false;
    await refreshCollection();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    itemSaving.value = false;
  }
}

async function removeItem(entry: CollectionEntry) {
  try {
    await deleteCollectionItem(entry.id!);
    ElMessage.success("已删除");
    await refreshCollection();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

/** 按来源前缀路由到对应安装通道 */
function installFromCollection(entry: CollectionEntry) {
  if (entry.source.startsWith("skills.sh:")) {
    openInstall("skills_sh", entry.source.slice("skills.sh:".length), entry.name);
  } else if (entry.source.startsWith("git:")) {
    openInstall("git", entry.source.slice("git:".length), entry.name);
  } else if (entry.source.startsWith("library:")) {
    openInstall("library", entry.source.slice("library:".length), entry.name);
  } else {
    ElMessage.info("该条目无安装引用，仅作编目记录");
  }
}

/* ---------- 搜索 ---------- */

const source = ref<"skills.sh" | "skillsmp">("skills.sh");
const keyword = ref("");
const searching = ref(false);
const results = ref<MarketSkill[]>([]);
const searched = ref(false);

const QUICK_TAGS = ["git", "browser", "pdf", "frontend", "testing", "documentation", "database"];

function fmtCount(n: number | null): string {
  if (!n) return "—";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

async function doSearch(q?: string) {
  const query = (q ?? keyword.value).trim();
  if (!query) {
    ElMessage.warning("输入关键词搜索，或点击热门标签");
    return;
  }
  keyword.value = query;
  searching.value = true;
  searched.value = true;
  try {
    results.value = await marketSearch(source.value, query, 20);
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    searching.value = false;
  }
}

/* ---------- Git URL 直装 ---------- */

const gitUrl = ref("");
const gitInstalling = ref(false);

/* ---------- 详情抽屉（skills.sh） ---------- */

const previewVisible = ref(false);
const previewTitle = ref("");
const previewId = ref("");
const preview = ref<MarketPreview | null>(null);
const previewLoading = ref(false);

async function openPreview(row: MarketSkill) {
  previewTitle.value = row.name;
  previewId.value = row.id;
  preview.value = null;
  previewVisible.value = true;
  previewLoading.value = true;
  try {
    preview.value = await marketPreview(row.id);
  } catch (e) {
    ElMessage.error(String(e));
    previewVisible.value = false;
  } finally {
    previewLoading.value = false;
  }
}

/* ---------- 安装对话框（市场 / Git / 中央库共用） ---------- */

const installDialogVisible = ref(false);
const installing = ref(false);
const installTarget = reactive<{
  kind: "skills_sh" | "git" | "library";
  label: string;
  id: string; // skills.sh id / git url / library name
}>({ kind: "skills_sh", label: "", id: "" });
const installAgents = ref<string[]>([]);
const overwrite = ref(false);
const installResult = ref<{ adopt: AdoptReport; deploys: DeployResult[] } | null>(null);
const libraryOnlyResult = ref<DeployResult[] | null>(null);

function openInstall(kind: "skills_sh" | "git" | "library", id: string, label: string) {
  installTarget.kind = kind;
  installTarget.id = id;
  installTarget.label = label;
  installAgents.value = installedAgents.value.slice(0, 1).map((a) => a.id);
  overwrite.value = false;
  installResult.value = null;
  libraryOnlyResult.value = null;
  installDialogVisible.value = true;
}

function openGitInstall() {
  const url = gitUrl.value.trim();
  if (!url || !url.includes("://")) {
    ElMessage.warning("请粘贴有效的 Git 仓库或子目录链接");
    return;
  }
  openInstall("git", url, url);
}

async function confirmInstall() {
  if (!installAgents.value.length) {
    ElMessage.warning("请至少勾选一个要安装到的 Agent");
    return;
  }
  installing.value = true;
  try {
    if (installTarget.kind === "library") {
      const deploys = await deployLibrarySkill(installTarget.id, installAgents.value, overwrite.value);
      libraryOnlyResult.value = deploys;
      const ok = deploys.filter((d) => d.ok).length;
      if (ok) ElMessage.success(`已安装到 ${ok} 个 Agent`);
      deploys.filter((d) => !d.ok).forEach((d) => ElMessage.error(d.error ?? "失败"));
    } else {
      const outcome =
        installTarget.kind === "skills_sh"
          ? await marketInstallSkillsSh(installTarget.id, installAgents.value, overwrite.value)
          : await marketInstallGit(installTarget.id, installAgents.value, overwrite.value);
      installResult.value = { adopt: outcome.adopt, deploys: outcome.deploys };
      const ok = outcome.deploys.filter((d) => d.ok).length;
      if (ok) ElMessage.success(`已入库（${outcome.adopt.files.length} 个文件）并安装到 ${ok} 个 Agent`);
      outcome.deploys.filter((d) => !d.ok).forEach((d) => ElMessage.error(d.error ?? "失败"));
      await refreshLibrary();
    }
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    installing.value = false;
  }
}

/* ---------- 中央库 ---------- */

const library = ref<LibraryItem[]>([]);
const libraryLoading = ref(false);

async function refreshLibrary() {
  libraryLoading.value = true;
  try {
    library.value = await listLibrary();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    libraryLoading.value = false;
  }
}

const agentName = (id: string) => agents.value.find((a) => a.id === id)?.name ?? id;

onMounted(() => {
  agentsStore.fetchAll().catch(() => undefined);
  refreshLibrary();
  refreshCollection();
});
</script>

<template>
  <div>
    <el-tabs v-model="activeTab">
      <el-tab-pane label="市场" name="market">
        <!-- Git URL 直装 -->
        <div class="url-row">
          <el-input v-model="gitUrl" :prefix-icon="Link" placeholder="粘贴 Git 链接：仓库根、/tree/branch/子目录 或 URL#子路径" clearable>
            <template #append>
              <el-button :icon="Download" :loading="gitInstalling" @click="openGitInstall">安装</el-button>
            </template>
          </el-input>
        </div>

        <!-- 搜索 -->
        <div class="search-row">
          <el-radio-group v-model="source">
            <el-radio-button value="skills.sh">skills.sh</el-radio-button>
            <el-radio-button value="skillsmp">SkillsMP（语义）</el-radio-button>
          </el-radio-group>
          <el-input
            v-model="keyword"
            :prefix-icon="Search"
            :placeholder="source === 'skills.sh' ? '搜索 skills.sh 市场' : '按功能搜索（配 SkillsMP 密钥效果最佳，见设置页）'"
            clearable
            class="kw"
            @keyup.enter="doSearch()"
          />
          <el-button type="primary" :loading="searching" @click="doSearch()">搜索</el-button>
        </div>

        <div class="tags" v-if="!searched">
          <span class="tags-label">热门：</span>
          <el-tag
            v-for="t in QUICK_TAGS"
            :key="t"
            class="tag"
            effect="plain"
            style="cursor: pointer"
            @click="doSearch(t)"
          >{{ t }}</el-tag>
        </div>

        <el-table :data="results" v-loading="searching" stripe>
          <el-table-column label="Skill" min-width="170">
            <template #default="{ row }">
              <span class="skill-name">{{ row.name }}</span>
              <el-tag size="small" effect="plain" class="ml">{{ row.market }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="来源" width="150">
            <template #default="{ row }">{{ row.author ?? row.source ?? "—" }}</template>
          </el-table-column>
          <el-table-column label="描述" min-width="300">
            <template #default="{ row }">
              <span class="desc">{{ row.description ?? "—（点详情查看）" }}</span>
            </template>
          </el-table-column>
          <el-table-column label="热度" width="110">
            <template #default="{ row }">
              <span v-if="row.installs">{{ fmtCount(row.installs) }} 安装</span>
              <span v-else-if="row.stars">★ {{ fmtCount(row.stars) }}</span>
              <span v-else>—</span>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="160" fixed="right">
            <template #default="{ row }">
              <el-button
                v-if="row.market === 'skills.sh'"
                size="small"
                :icon="View"
                @click="openPreview(row)"
              >详情</el-button>
              <el-button
                size="small"
                type="primary"
                :icon="Download"
                @click="openInstall(row.market === 'skills.sh' ? 'skills_sh' : 'git', row.market === 'skills.sh' ? row.id : (row.githubUrl ?? row.id), row.name)"
              >安装</el-button>
            </template>
          </el-table-column>
          <template #empty>输入关键词或点击热门标签开始浏览</template>
        </el-table>
      </el-tab-pane>

      <el-tab-pane :label="`中央库（${library.length}）`" name="library">
        <el-alert
          title="市场安装与「收编」的 skill 都在这里；选中后一键安装到任意 Agent 的用户级 skill 目录"
          type="info"
          :closable="false"
          class="mb"
        />
        <div class="lib-toolbar">
          <el-button :icon="Refresh" size="small" :loading="libraryLoading" @click="refreshLibrary">刷新</el-button>
        </div>
        <el-table :data="library" v-loading="libraryLoading" stripe>
          <el-table-column prop="name" label="Skill" min-width="160" />
          <el-table-column label="来源" width="220">
            <template #default="{ row }">{{ row.sourceAgent ?? "—" }}</template>
          </el-table-column>
          <el-table-column label="描述" min-width="240">
            <template #default="{ row }">{{ row.description ?? "—" }}</template>
          </el-table-column>
          <el-table-column prop="fileCount" label="文件数" width="90" />
          <el-table-column label="操作" width="130" fixed="right">
            <template #default="{ row }">
              <el-button size="small" type="primary" plain :icon="Download" @click="openInstall('library', row.name, row.name)">安装到…</el-button>
            </template>
          </el-table-column>
          <template #empty>中央库还是空的——去市场安装，或在 Skill 中心收编现有 skill</template>
        </el-table>
      </el-tab-pane>
      <el-tab-pane :label="`收藏集（${curated.length}+${myItems.length}）`" name="collection">
        <el-alert
          title="内置精选随软件版本更新（条目均经真机验证）；「我的条目」保存在本机 SQLite"
          type="info"
          :closable="false"
          class="mb"
        />
        <div class="lib-toolbar">
          <span class="section-sub">内置精选</span>
        </div>
        <el-table :data="curated" v-loading="collectionLoading" stripe size="small" class="mb">
          <el-table-column prop="name" label="Skill" min-width="150" />
          <el-table-column label="标签" min-width="140">
            <template #default="{ row }">
              <el-tag v-for="t in row.tags" :key="t" size="small" effect="plain" class="agent-tag">{{ t }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="note" label="说明" min-width="260" />
          <el-table-column label="操作" width="100" fixed="right">
            <template #default="{ row }">
              <el-button size="small" type="primary" plain :icon="Download" @click="installFromCollection(row)">安装</el-button>
            </template>
          </el-table-column>
        </el-table>

        <div class="lib-toolbar">
          <span class="section-sub">我的条目</span>
          <el-button size="small" type="primary" :icon="FolderAdd" @click="openNewItem">添加条目</el-button>
        </div>
        <el-table :data="myItems" v-loading="collectionLoading" stripe size="small">
          <el-table-column prop="name" label="名称" min-width="140" />
          <el-table-column prop="kind" label="类型" width="80" />
          <el-table-column prop="source" label="来源" min-width="220">
            <template #default="{ row }">
              <code class="cmd">{{ row.source }}</code>
            </template>
          </el-table-column>
          <el-table-column label="标签" min-width="120">
            <template #default="{ row }">
              <el-tag v-for="t in row.tags" :key="t" size="small" effect="plain" class="agent-tag">{{ t }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="星级" width="120">
            <template #default="{ row }">
              <el-rate :model-value="row.stars" disabled size="small" />
            </template>
          </el-table-column>
          <el-table-column label="操作" width="180" fixed="right">
            <template #default="{ row }">
              <el-button size="small" :icon="Download" @click="installFromCollection(row)">安装</el-button>
              <el-button size="small" :icon="Edit" @click="openEditItem(row)" />
              <el-button size="small" type="danger" plain :icon="Delete" @click="removeItem(row)" />
            </template>
          </el-table-column>
          <template #empty>还没有自定义条目——工具、仓库、常用组合都可以编目在这里</template>
        </el-table>
      </el-tab-pane>
    </el-tabs>

    <!-- 收藏条目编辑对话框 -->
    <el-dialog v-model="itemDialogVisible" :title="itemForm.id == null ? '添加收藏条目' : '编辑收藏条目'" width="460px">
      <el-form label-position="top">
        <el-form-item label="类型">
          <el-radio-group v-model="itemForm.kind">
            <el-radio-button value="skill">skill</el-radio-button>
            <el-radio-button value="mcp">mcp</el-radio-button>
            <el-radio-button value="tool">tool</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="名称" required>
          <el-input v-model="itemForm.name" />
        </el-form-item>
        <el-form-item label="来源引用（skills.sh:xxx / git:URL / library:名称）" required>
          <el-input v-model="itemForm.source" placeholder="git:https://github.com/owner/repo" />
        </el-form-item>
        <el-form-item label="标签（逗号分隔）">
          <el-input v-model="itemForm.tags" placeholder="git, 提交" />
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="itemForm.note" type="textarea" :rows="2" />
        </el-form-item>
        <el-form-item label="星级">
          <el-rate v-model="itemForm.stars" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="itemDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="itemSaving" @click="saveItem">保存</el-button>
      </template>
    </el-dialog>

    <!-- skills.sh 详情抽屉 -->
    <el-drawer v-model="previewVisible" :title="previewTitle" size="480px">
      <div v-loading="previewLoading">
        <template v-if="preview">
          <p class="desc-block">{{ preview.description ?? "（SKILL.md 未提供描述）" }}</p>
          <p class="mb">包含 {{ preview.files.length }} 个文件：</p>
          <pre class="files">{{ preview.files.join("\n") }}</pre>
          <el-button type="primary" class="w-full" :icon="Download" @click="openInstall('skills_sh', previewId, previewTitle)">
            下一步：选择 Agent 安装
          </el-button>
        </template>
      </div>
    </el-drawer>

    <!-- 安装对话框 -->
    <el-dialog v-model="installDialogVisible" title="安装 Skill" width="460px">
      <p class="mb"><b>{{ installTarget.label }}</b></p>
      <p class="mb">
        {{ installTarget.kind === "library" ? "将复制到所选 Agent 的用户级 skill 目录。" : "将先下载到中央库，再复制到所选 Agent 的用户级 skill 目录。" }}
      </p>
      <el-form label-position="top">
        <el-form-item label="安装到" required>
          <el-checkbox-group v-model="installAgents">
            <el-checkbox v-for="a in installedAgents" :key="a.id" :value="a.id" :label="a.name" />
          </el-checkbox-group>
          <div v-if="!installedAgents.length" class="hint">未检测到已安装的 Agent</div>
        </el-form-item>
        <el-form-item>
          <el-switch v-model="overwrite" active-text="目标已存在时覆盖" />
        </el-form-item>
      </el-form>

      <template v-if="installResult || libraryOnlyResult">
        <el-divider />
        <p class="mb" v-if="installResult">入库：{{ installResult.adopt.files.length }} 个文件（{{ installResult.adopt.skillName }}）</p>
        <div v-for="d in (installResult?.deploys ?? libraryOnlyResult ?? [])" :key="d.agentId" class="deploy-line">
          <el-tag size="small" :type="d.ok ? 'success' : 'danger'">{{ agentName(d.agentId) }}</el-tag>
          <span class="deploy-err">{{ d.error ?? "完成" }}</span>
        </div>
      </template>

      <template #footer>
        <el-button @click="installDialogVisible = false">关闭</el-button>
        <el-button type="primary" :loading="installing" @click="confirmInstall">执行安装</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.url-row { margin-bottom: 12px; }
.search-row { display: flex; gap: 10px; margin-bottom: 12px; }
.kw { flex: 1; }
.tags { margin-bottom: 12px; display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.tags-label { font-size: 12px; color: var(--el-text-color-secondary); }
.tag { cursor: pointer; }
.ml { margin-left: 6px; }
.mb { margin-bottom: 12px; }
.skill-name { font-weight: 600; }
.desc { font-size: 12px; color: var(--el-text-color-regular); }
.desc-block { margin-bottom: 14px; line-height: 1.7; }
.files {
  font-size: 12px; font-family: Consolas, monospace;
  background: var(--el-fill-color-light); border-radius: 6px;
  padding: 12px; max-height: 40vh; overflow: auto; margin-bottom: 14px;
}
.w-full { width: 100%; }
.lib-toolbar { margin-bottom: 10px; display: flex; justify-content: space-between; align-items: center; }
.section-sub { font-weight: 600; }
.hint { font-size: 12px; color: var(--el-text-color-secondary); }
.deploy-line { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
.deploy-err { font-size: 12px; color: var(--el-text-color-secondary); word-break: break-all; }
</style>
