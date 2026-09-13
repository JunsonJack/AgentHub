<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { DeleteLocation, Key, Refresh } from "@element-plus/icons-vue";
import { listSnapshots, rollbackSnapshot, skillsmpClearKey, skillsmpKeyStatus, skillsmpSetKey } from "../api";
import type { SnapshotMeta } from "../api/types";

const snapshots = ref<SnapshotMeta[]>([]);
const loading = ref(false);
const filter = ref("");

const filtered = computed(() =>
  snapshots.value.filter(
    (s) =>
      !filter.value.trim() ||
      s.originalPath.toLowerCase().includes(filter.value.trim().toLowerCase()) ||
      s.fileName.toLowerCase().includes(filter.value.trim().toLowerCase())
  )
);

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
});
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
      <span class="section-title">快照历史（{{ snapshots.length }}）</span>
      <div>
        <el-input v-model="filter" placeholder="按原路径过滤" clearable size="small" class="filter" />
        <el-button size="small" :icon="Refresh" :loading="loading" @click="refresh">刷新</el-button>
      </div>
    </div>

    <el-table :data="filtered" v-loading="loading" stripe>
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
</template>

<style scoped>
.mb { margin-bottom: 20px; }
.section-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
.section-title { font-weight: 600; }
.filter { width: 240px; margin-right: 8px; }
.cmd {
  font-size: 12px; font-family: Consolas, monospace;
  background: var(--el-fill-color); padding: 2px 6px; border-radius: 4px;
}
.key-row { display: flex; gap: 10px; margin-bottom: 10px; }
.key-input { max-width: 420px; }
.key-note { font-size: 12px; color: var(--el-text-color-secondary); line-height: 1.7; }
</style>
