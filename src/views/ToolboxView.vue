<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { CopyDocument, Delete, Edit, Plus, Refresh, Search, TopRight } from "@element-plus/icons-vue";
import {
  addCollectionItem,
  deleteCollectionItem,
  listCollection,
  updateCollectionItem,
} from "../api";
import type { CollectionEntry } from "../api/types";
import { openExternal } from "../utils/external";
import { copyText } from "../utils/clipboard";

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

/** 非链接入口（按钮 / 键盘）用的显式打开；点 <a> 时交给全局守卫，不重复挂处理器 */
function openTool(item: CollectionEntry) {
  void openExternal(item.source);
}

/** 非 http(s) 的条目（如 CLI 命令、本地路径）不能当链接看 */
function isWebUrl(source: string): boolean {
  return /^https?:\/\//i.test(source.trim());
}

function copySource(item: CollectionEntry) {
  void copyText(item.source, `已复制「${item.name}」链接`);
}

onMounted(refresh);

/* ---------- 书签导入（Netscape Bookmark HTML） ---------- */

const bookmarkDialogVisible = ref(false);
const bookmarkHtml = ref("");
const bookmarkFolderTag = ref("");
const bookmarkImporting = ref(false);
const bookmarkFileInput = ref<HTMLInputElement | null>(null);

function openBookmarkImport() {
  bookmarkHtml.value = "";
  bookmarkFolderTag.value = "";
  bookmarkDialogVisible.value = true;
}

async function onBookmarkFile(ev: Event) {
  const input = ev.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try {
    bookmarkHtml.value = await file.text();
    ElMessage.success(`已读取 ${file.name}（${(file.size / 1024).toFixed(1)} KB）`);
  } catch (e) {
    ElMessage.error(String(e));
  }
  input.value = "";
}

interface ParsedBookmark {
  name: string;
  url: string;
  folder: string;
}

function parseNetscapeBookmarks(html: string): ParsedBookmark[] {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const anchors = [...doc.querySelectorAll("a[href]")];
  const out: ParsedBookmark[] = [];
  const seen = new Set<string>();
  for (const a of anchors) {
    const url = (a.getAttribute("href") || "").trim();
    if (!url || url.startsWith("javascript:") || url.startsWith("place:")) continue;
    const name = (a.textContent || "").trim() || url;
    // 最近文件夹：向上找 DL 的前驱 H3 / DT>H3
    let folder = "";
    let node: Element | null = a.parentElement;
    while (node) {
      if (node.tagName === "DL") {
        const prev = node.previousElementSibling;
        if (prev && prev.tagName === "H3") {
          folder = (prev.textContent || "").trim();
          break;
        }
        if (prev?.tagName === "DT") {
          const h = prev.querySelector("H3");
          if (h) {
            folder = (h.textContent || "").trim();
            break;
          }
        }
      }
      node = node.parentElement;
    }
    const key = url.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ name, url, folder });
  }
  return out;
}

async function confirmBookmarkImport() {
  const html = bookmarkHtml.value.trim();
  if (!html) {
    ElMessage.warning("请粘贴书签 HTML 内容，或选择书签文件");
    return;
  }
  bookmarkImporting.value = true;
  try {
    const parsed = parseNetscapeBookmarks(html);
    if (!parsed.length) {
      ElMessage.warning("未解析到有效书签（需 Netscape 格式：Chrome/Edge/Firefox 导出的 bookmarks.html）");
      return;
    }
    const extraTag = bookmarkFolderTag.value.trim();
    let ok = 0;
    let skip = 0;
    const existing = new Set(items.value.map((i) => i.source.toLowerCase()));
    for (const b of parsed) {
      if (existing.has(b.url.toLowerCase())) {
        skip += 1;
        continue;
      }
      const tags = ["bookmark"];
      if (b.folder) tags.push(b.folder);
      if (extraTag) tags.push(extraTag);
      await addCollectionItem({
        id: null,
        kind: "tool",
        name: b.name.slice(0, 80),
        source: b.url,
        tags,
        note: b.folder ? `来自书签文件夹「${b.folder}」` : "来自浏览器书签导入",
        stars: 0,
        builtIn: false,
      });
      existing.add(b.url.toLowerCase());
      ok += 1;
    }
    ElMessage.success(`已导入 ${ok} 条${skip ? `，跳过 ${skip} 条重复链接` : ""}`);
    bookmarkDialogVisible.value = false;
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    bookmarkImporting.value = false;
  }
}
</script>

<template>
  <div>
    <div class="toolbar">
      <el-input v-model="keyword" :prefix-icon="Search" placeholder="搜索名称 / 链接 / 笔记" clearable class="kw" />
      <el-select v-model="tagFilter" placeholder="全部标签" clearable class="sel">
        <el-option v-for="t in allTags" :key="t" :label="t" :value="t" />
      </el-select>
      <el-button type="primary" :icon="Plus" @click="openNew">添加工具</el-button>
      <el-button @click="openBookmarkImport">导入书签</el-button>
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
              <template v-if="isWebUrl(item.source)">
                <el-tooltip content="在系统默认浏览器打开（不会离开本应用）" placement="top">
                  <a class="tool-name" :href="item.source">
                    <span class="tool-name-text">{{ item.name }}</span>
                    <el-icon class="ext-icon"><TopRight /></el-icon>
                  </a>
                </el-tooltip>
              </template>
              <el-tooltip v-else content="非网页地址（CLI / 本地路径），已按原样展示" placement="top">
                <span class="tool-name is-plain">{{ item.name }}</span>
              </el-tooltip>
              <div class="card-ops">
                <el-button
                  v-if="isWebUrl(item.source)"
                  size="small"
                  text
                  :icon="TopRight"
                  title="在浏览器打开"
                  @click="openTool(item)"
                />
                <el-button size="small" text :icon="CopyDocument" title="复制链接" @click="copySource(item)" />
                <el-button size="small" text :icon="Edit" title="编辑" @click="openEdit(item)" />
                <el-button size="small" text type="danger" :icon="Delete" title="删除" @click="remove(item)" />
              </div>
            </div>
          </template>
          <p class="note">{{ item.note || "（无笔记）" }}</p>
          <div class="tag-row">
            <el-tag v-for="t in item.tags" :key="t" size="small" effect="plain" class="tag">{{ t }}</el-tag>
          </div>
          <div class="bottom">
            <el-rate :model-value="item.stars" disabled size="small" />
            <a v-if="isWebUrl(item.source)" class="url" :href="item.source" :title="item.source">{{ item.source }}</a>
            <span v-else class="url" :title="item.source">{{ item.source }}</span>
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

    <!-- 书签导入 -->
    <el-dialog v-model="bookmarkDialogVisible" title="从浏览器书签导入" width="560px">
      <el-alert
        type="info"
        :closable="false"
        title="浏览器书签管理器 → 导出书签 → 得到 bookmarks.html"
        description="支持 Chrome / Edge / Firefox 导出的 Netscape 格式。可选择文件，或直接粘贴 HTML 内容。重复链接会跳过。"
        class="mb"
      />
      <div class="bookmark-actions mb">
        <el-button @click="bookmarkFileInput?.click()">选择书签文件</el-button>
        <input ref="bookmarkFileInput" type="file" accept=".html,.htm,text/html" class="hidden-file" @change="onBookmarkFile" />
        <el-input v-model="bookmarkFolderTag" placeholder="附加标签（可选，如 个人）" class="tag-input" />
      </div>
      <el-input
        v-model="bookmarkHtml"
        type="textarea"
        :rows="8"
        spellcheck="false"
        class="mono"
        placeholder="<!DOCTYPE NETSCAPE-Bookmark-file-1> ..."
      />
      <template #footer>
        <el-button @click="bookmarkDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="bookmarkImporting" @click="confirmBookmarkImport">解析并导入</el-button>
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
.card-ops { display: flex; align-items: center; flex: none; }
.tool-name {
  display: inline-flex; align-items: center; gap: 4px; min-width: 0;
  font-weight: 600; color: var(--el-color-primary); text-decoration: none; cursor: pointer;
}
.tool-name-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tool-name:hover .tool-name-text { text-decoration: underline; }
.tool-name.is-plain { color: var(--el-text-color-primary); cursor: default; }
.ext-icon { font-size: 12px; opacity: 0.7; flex: none; }
.note {
  font-size: 12px; color: var(--el-text-color-regular); line-height: 1.6;
  min-height: 38px; margin-bottom: 8px;
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
}
.tag-row { margin-bottom: 10px; display: flex; flex-wrap: wrap; gap: 4px; }
.bottom { display: flex; justify-content: space-between; align-items: center; }
.url {
  font-size: 11px; color: var(--el-text-color-secondary); text-decoration: none;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 60%;
}
a.url:hover { color: var(--el-color-primary); text-decoration: underline; }
.url-row { display: flex; gap: 8px; width: 100%; }
.mb { margin-bottom: 12px; }
.bookmark-actions { display: flex; gap: 10px; align-items: center; }
.tag-input { width: 200px; }
.hidden-file { display: none; }
.mono :deep(textarea) { font-family: Consolas, monospace; }
</style>
