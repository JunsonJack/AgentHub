<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { storeToRefs } from "pinia";
import { ElMessage, ElMessageBox } from "element-plus";
import { Delete, Edit, Refresh, Search, Upload } from "@element-plus/icons-vue";
import { useAgentsStore } from "../stores/agents";
import { deployMcp, removeMcp } from "../api";
import type { McpEntry, McpServerDef } from "../api/types";

const store = useAgentsStore();
const { mcp, agents, loading, loaded, error } = storeToRefs(store);

const keyword = ref("");
const agentFilter = ref("");

const agentName = (id: string) => agents.value.find((a) => a.id === id)?.name ?? id;

const filtered = computed(() =>
  mcp.value.filter((e) => {
    const okAgent = !agentFilter.value || e.agentId === agentFilter.value;
    const kw = keyword.value.trim().toLowerCase();
    const okKw =
      !kw ||
      e.name.toLowerCase().includes(kw) ||
      (e.command ?? "").toLowerCase().includes(kw) ||
      (e.url ?? "").toLowerCase().includes(kw);
    return okAgent && okKw;
  })
);

const transportTag = (t: string) => (t === "stdio" ? "success" : t === "unknown" ? "info" : "warning");
const scopeLabel = (s: string) => (s === "global" ? "全局" : s.startsWith("project:") ? "项目" : s);

async function refresh() {
  try {
    await store.fetchAll();
  } catch {
    ElMessage.error(error.value || "读取 MCP 配置失败");
  }
}

/* ---------- 编辑 / 下发抽屉 ---------- */

const drawerVisible = ref(false);
const editMode = ref<"form" | "source">("form");
const isNew = ref(true);
const form = reactive({
  name: "",
  command: "",
  args: "",
  env: "",
  url: "",
  agents: [] as string[],
});
const sourceJson = ref("");
/** 编辑来源（用于区分新增 / 修改，以及默认勾选的 Agent） */
const originEntry = ref<McpEntry | null>(null);

function parseArgs(s: string): string[] {
  return s
    .split(/[\s,]+/)
    .map((x) => x.trim())
    .filter(Boolean);
}

function parseEnv(s: string): Record<string, unknown> | undefined {
  const t = s.trim();
  if (!t) return undefined;
  const parsed = JSON.parse(t) as Record<string, unknown>;
  if (typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("env 必须是 JSON 对象");
  return parsed;
}

function buildDef(): McpServerDef {
  if (editMode.value === "source") {
    const obj = JSON.parse(sourceJson.value) as McpServerDef;
    // name 不在 def 里，随下发参数传递
    delete (obj as Record<string, unknown>).name;
    return obj;
  }
  const def: McpServerDef = {};
  if (form.url.trim()) {
    def.url = form.url.trim();
  } else if (form.command.trim()) {
    def.command = form.command.trim();
    const args = parseArgs(form.args);
    if (args.length) def.args = args;
  }
  const env = parseEnv(form.env);
  if (env) def.env = env;
  return def;
}

function validateDef(def: McpServerDef): string | null {
  if (!def.command && !def.url) return "必须填写 command（stdio）或 url（http/sse）之一";
  if (def.command && def.url) return "command 与 url 只能填一个";
  return null;
}

function openNew() {
  isNew.value = true;
  originEntry.value = null;
  editMode.value = "form";
  Object.assign(form, { name: "", command: "", args: "", env: "", url: "", agents: [] });
  sourceJson.value = "{}";
  drawerVisible.value = true;
}

function openEdit(entry: McpEntry) {
  isNew.value = false;
  originEntry.value = entry;
  editMode.value = "form";
  form.name = entry.name;
  form.command = entry.command ?? "";
  form.args = entry.args.join(" ");
  form.url = entry.url ?? "";
  form.env = entry.raw.env ? JSON.stringify(entry.raw.env, null, 2) : "";
  form.agents = [entry.agentId];
  sourceJson.value = JSON.stringify(entry.raw, null, 2);
  drawerVisible.value = true;
}

async function save() {
  const name = form.name.trim();
  if (!name) {
    ElMessage.warning("请填写 server 名称");
    return;
  }
  if (!form.agents.length) {
    ElMessage.warning("请至少勾选一个要安装到的 Agent");
    return;
  }
  let def: McpServerDef;
  try {
    def = buildDef();
  } catch (e) {
    ElMessage.error(`配置解析失败：${e instanceof Error ? e.message : String(e)}`);
    return;
  }
  const problem = validateDef(def);
  if (problem) {
    ElMessage.warning(problem);
    return;
  }
  try {
    const results = await deployMcp(form.agents, name, def);
    const okAgents = results.filter((r) => r.ok).map((r) => agentName(r.agentId));
    const failed = results.filter((r) => !r.ok);
    if (okAgents.length) ElMessage.success(`已写入：${okAgents.join("、")}`);
    failed.forEach((f) => ElMessage.error(`${agentName(f.agentId)} 失败：${f.error}`));
    drawerVisible.value = false;
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function onDelete(entry: McpEntry) {
  try {
    await ElMessageBox.confirm(
      `将从 ${agentName(entry.agentId)} 的配置中删除 ${entry.name}。删除前会自动快照，可随时回滚。`,
      "确认删除",
      { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" }
    );
  } catch {
    return;
  }
  try {
    await removeMcp(entry.agentId, entry.name, entry.scope);
    ElMessage.success("已删除");
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
      <el-input v-model="keyword" :prefix-icon="Search" placeholder="搜索 server 名 / command / url" clearable class="kw" />
      <el-select v-model="agentFilter" placeholder="全部 Agent" clearable class="agent-select">
        <el-option v-for="a in agents" :key="a.id" :label="a.name" :value="a.id" />
      </el-select>
      <el-button type="primary" :icon="Upload" @click="openNew">新增 / 下发</el-button>
      <el-button :icon="Refresh" :loading="loading" @click="refresh">刷新</el-button>
    </div>

    <el-alert
      v-if="loaded && mcp.length === 0"
      title="没有读到任何 MCP 配置"
      description="各 Agent 的配置文件不存在或为空。可以用「新增 / 下发」创建第一个条目。"
      type="info"
      :closable="false"
      class="mb"
    />

    <el-table :data="filtered" v-loading="loading && !loaded" stripe class="mcp-table">
      <el-table-column label="Server" min-width="180">
        <template #default="{ row }">
          <span class="server-name">{{ row.name }}</span>
        </template>
      </el-table-column>
      <el-table-column label="Agent" width="130">
        <template #default="{ row }">{{ agentName(row.agentId) }}</template>
      </el-table-column>
      <el-table-column label="作用域" width="90">
        <template #default="{ row }">
          <el-tag size="small" :type="row.scope === 'global' ? 'success' : 'warning'" effect="plain">
            {{ scopeLabel(row.scope) }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="传输" width="90">
        <template #default="{ row }">
          <el-tag size="small" :type="transportTag(row.transport)" effect="plain">{{ row.transport }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="启动命令 / URL" min-width="240">
        <template #default="{ row }">
          <code class="cmd">{{ row.command ?? row.url ?? "—" }}</code>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="140" fixed="right">
        <template #default="{ row }">
          <el-button
            size="small"
            :icon="Edit"
            :disabled="row.scope !== 'global'"
            :title="row.scope !== 'global' ? '项目级条目暂不支持编辑，请在对应项目内修改' : ''"
            @click="openEdit(row)"
          >编辑</el-button>
          <el-button
            size="small"
            type="danger"
            plain
            :icon="Delete"
            :disabled="row.scope !== 'global'"
            @click="onDelete(row)"
          />
        </template>
      </el-table-column>
    </el-table>

    <el-drawer v-model="drawerVisible" :title="isNew ? '新增 MCP server 并下发' : `编辑：${form.name}`" size="520px">
      <el-form label-position="top">
        <el-form-item label="Server 名称" required>
          <el-input v-model="form.name" :disabled="!isNew" placeholder="如 yapi" />
        </el-form-item>

        <el-radio-group v-model="editMode" class="mb">
          <el-radio-button value="form">表单模式</el-radio-button>
          <el-radio-button value="source">源码模式</el-radio-button>
        </el-radio-group>

        <template v-if="editMode === 'form'">
          <el-form-item label="command（stdio 传输）">
            <el-input v-model="form.command" placeholder="如 npx / uvx / node" />
          </el-form-item>
          <el-form-item label="args（空格或逗号分隔）">
            <el-input v-model="form.args" placeholder="如 -y some-mcp-server" />
          </el-form-item>
          <el-form-item label="url（http / sse 传输）">
            <el-input v-model="form.url" placeholder="https://..." />
          </el-form-item>
          <el-form-item label="env（JSON 对象，可选）">
            <el-input v-model="form.env" type="textarea" :rows="4" placeholder='{ "API_KEY": "..." }' />
          </el-form-item>
        </template>
        <template v-else>
          <el-form-item label="原始 JSON（未知字段会原样保留）">
            <el-input v-model="sourceJson" type="textarea" :rows="12" spellcheck="false" class="mono" />
          </el-form-item>
        </template>

        <el-form-item label="安装到（写入前自动快照）" required>
          <el-checkbox-group v-model="form.agents">
            <el-checkbox v-for="a in agents" :key="a.id" :value="a.id" :label="a.name" />
          </el-checkbox-group>
        </el-form-item>

        <el-button type="primary" class="save-btn" @click="save">保存并下发</el-button>
      </el-form>
    </el-drawer>
  </div>
</template>

<style scoped>
.toolbar { display: flex; gap: 10px; margin-bottom: 14px; }
.kw { width: 320px; }
.agent-select { width: 180px; }
.mb { margin-bottom: 12px; }
.server-name { font-weight: 600; }
.mcp-table { background: #fff; }
.cmd {
  font-size: 12px; font-family: Consolas, monospace;
  background: var(--el-fill-color); padding: 2px 6px; border-radius: 4px;
}
.mono :deep(textarea) { font-family: Consolas, monospace; }
.save-btn { width: 100%; }
</style>
