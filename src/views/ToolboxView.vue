<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { Delete, Edit, Plus, Refresh, Search } from "@element-plus/icons-vue";
import {
  addCollectionItem,
  deleteCollectionItem,
  listCollection,
  updateCollectionItem,
} from "../api";
import type { CollectionEntry } from "../api/types";

const items = ref<CollectionEntry[]>([]);
const loading = ref(false);
const keyword = ref("");
const tagFilter = ref("");

const allTags = computed(() => {
  const set = new Set<string>();
  items.value.forEach((i) => i.tags.forEach((t) => set.add(t)));
  return [...set].sort();
});

const filtered = computed(() =>
  items.value.filter((i) => {
    const okTag = !tagFilter.value || i.tags.includes(tagFilter.value);
    const kw = keyword.value.trim().toLowerCase();
    const okKw =
      !kw ||
      i.name.toLowerCase().includes(kw) ||
      i.source.toLowerCase().includes(kw) ||
      i.note.toLowerCase().includes(kw);
    return okTag && okKw;
  })
);

async function refresh() {
  loading.value = true;
  try {
    items.value = (await listCollection()).filter((i) => i.kind === "tool");
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    loading.value = false;
  }
}

/* ---------- 添加 / 编辑 ---------- */

const dialogVisible = ref(false);
const saving = ref(false);
const form = reactive({
  id: null as number | null,
  name: "",
  source: "",
  tags: "",
  note: "",
  stars: 0,
});
const fetching = ref(false);

function openNew() {
  Object.assign(form, { id: null, name: "", source: "", tags: "", note: "", stars: 0 });
  dialogVisible.value = true;
}

function openEdit(item: CollectionEntry) {
  Object.assign(form, {
    id: item.id,
    name: item.name,
    source: item.source,
    tags: item.tags.join(", "),
    note: item.note,
    stars: item.stars,
  });
  dialogVisible.value = true;
}

async function fetchMeta() {
  const url = form.source.trim();
  if (!url.startsWith("http")) {
    ElMessage.warning("先填写以 http(s):// 开头的链接再抓取");
    return;
  }
  fetching.value = true;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const meta = await invoke<{ title: string; description: string }>("fetch_url_metadata", { url });
    if (meta.title && !form.name.trim()) form.name = meta.title;
    if (meta.description && !form.note.trim()) form.note = meta.description;
    ElMessage.success("已抓取标题与描述");
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    fetching.value = false;
  }
}

async function save() {
  if (!form.name.trim() || !form.source.trim()) {
    ElMessage.warning("名称与链接必填");
    return;
  }
  saving.value = true;
  try {
    const entry: CollectionEntry = {
      id: form.id,
      kind: "tool",
      name: form.name.trim(),
      source: form.source.trim(),
      tags: form.tags.split(/[,，]/).map((t) => t.trim()).filter(Boolean),
      note: form.note.trim(),
      stars: Number(form.stars) || 0,
      builtIn: false,
    };
    if (entry.id == null) {
      await addCollectionItem(entry);
      ElMessage.success("已添加");
    } else {
      await updateCollectionItem(entry);
      ElMessage.success("已更新");
    }
    dialogVisible.value = false;
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    saving.value = false;
  }
}

async function remove(item: CollectionEntry) {
  try {
    await ElMessageBox.confirm(`删除「${item.name}」？`, "确认删除", { type: "warning" });
  } catch {
    return;
  }
  try {
    await deleteCollectionItem(item.id!);
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

onMounted(refresh);
</script>

<template>
  <div>
    <div class="toolbar">
      <el-input v-model="keyword" :prefix-icon="Search" placeholder="搜索名称 / 链接 / 笔记" clearable class="kw" />
      <el-select v-model="tagFilter" placeholder="全部标签" clearable class="sel">
        <el-option v-for="t in allTags" :key="t" :label="t" :value="t" />
      </el-select>
      <el-button type="primary" :icon="Plus" @click="openNew">添加工具</el-button>
      <el-button :icon="Refresh" :loading="loading" @click="refresh">刷新</el-button>
    </div>

    <el-empty v-if="!filtered.length && !loading" description="还没有收藏的 AI 工具——网页、CLI、桌面应用都可以收进来">
      <el-button type="primary" :icon="Plus" @click="openNew">添加第一个工具</el-button>
    </el-empty>

    <el-row :gutter="16">
      <el-col v-for="item in filtered" :key="item.id" :span="8" class="card-col">
        <el-card shadow="hover">
          <template #header>
            <div class="card-head">
              <a :href="item.source" class="tool-name">{{ item.name }}</a>
              <div>
                <el-button size="small" :icon="Edit" @click="openEdit(item)" />
                <el-button size="small" type="danger" plain :icon="Delete" @click="remove(item)" />
              </div>
            </div>
          </template>
          <p class="note">{{ item.note || "（无笔记）" }}</p>
          <div class="tag-row">
            <el-tag v-for="t in item.tags" :key="t" size="small" effect="plain" class="tag">{{ t }}</el-tag>
          </div>
          <div class="bottom">
            <el-rate :model-value="item.stars" disabled size="small" />
            <span class="url" :title="item.source">{{ item.source }}</span>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-dialog v-model="dialogVisible" :title="form.id == null ? '添加工具' : '编辑工具'" width="480px">
      <el-form label-position="top">
        <el-form-item label="链接" required>
          <div class="url-row">
            <el-input v-model="form.source" placeholder="https://..." />
            <el-button :loading="fetching" @click="fetchMeta">抓取元数据</el-button>
          </div>
        </el-form-item>
        <el-form-item label="名称" required>
          <el-input v-model="form.name" />
        </el-form-item>
        <el-form-item label="标签（逗号分隔）">
          <el-input v-model="form.tags" placeholder="CLI, 网页, 写作" />
        </el-form-item>
        <el-form-item label="使用笔记">
          <el-input v-model="form.note" type="textarea" :rows="3" />
        </el-form-item>
        <el-form-item label="星级">
          <el-rate v-model="form.stars" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="save">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.toolbar { display: flex; gap: 10px; margin-bottom: 16px; }
.kw { width: 280px; }
.sel { width: 150px; }
.card-col { margin-bottom: 16px; }
.card-head { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
.tool-name { font-weight: 600; color: var(--el-color-primary); text-decoration: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.note {
  font-size: 12px; color: var(--el-text-color-regular); line-height: 1.6;
  min-height: 38px; margin-bottom: 8px;
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
}
.tag-row { margin-bottom: 10px; display: flex; flex-wrap: wrap; gap: 4px; }
.bottom { display: flex; justify-content: space-between; align-items: center; }
.url { font-size: 11px; color: var(--el-text-color-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 60%; }
.url-row { display: flex; gap: 8px; width: 100%; }
</style>
