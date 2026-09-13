<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { storeToRefs } from "pinia";
import { ElMessage, ElMessageBox } from "element-plus";
import { Delete, Edit, Link, Refresh, Search, Share, SwitchButton, Upload } from "@element-plus/icons-vue";
import { useAgentsStore } from "../stores/agents";
import { deployMcp, disableMcp, enableMcp, listDisabledMcp, propagateMcp, removeMcp, testMcp, testMcpDef } from "../api";
import type { ConnectivityResult, DisabledRecord, McpEntry, McpServerDef } from "../api/types";

const store = useAgentsStore();
const { mcp, agents, loading, loaded, error } = storeToRefs(store);

const keyword = ref("");
const agentFilter = ref("");
const viewMode = ref<"flat" | "grouped">("flat");

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

/* ---------- 跨 Agent 同步（MCP） ---------- */

const syncVisible = ref(false);
const syncSourceEntry = ref<McpEntry | null>(null);
const syncTargetAgents = ref<string[]>([]);
const syncingMcp = ref(false);
const syncMcpResults = ref<import("../api/types").DeployResult[] | null>(null);

function openMcpSync(entry: McpEntry) {
  syncSourceEntry.value = entry;
  syncTargetAgents.value = [];
  syncMcpResults.value = null;
  syncVisible.value = true;
}

async function confirmMcpSync() {
  const entry = syncSourceEntry.value;
  if (!entry || !syncTargetAgents.value.length) {
    ElMessage.warning("请勾选目标 Agent");
    return;
  }
  syncingMcp.value = true;
  try {
    const results = await propagateMcp(entry.agentId, entry.name, syncTargetAgents.value);
    syncMcpResults.value = results;
    const ok = results.filter((r) => r.ok).length;
    if (ok) ElMessage.success(`已传播到 ${ok} 个 Agent（含 env 的密钥按规则处理）`);
    results.filter((r) => !r.ok).forEach((r) => ElMessage.error(`${agentName(r.agentId)}: ${r.error}`));
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    syncingMcp.value = false;
  }
}

/* ---------- 覆盖矩阵（按 server 去重分组） ---------- */

interface GroupedServer {
  name: string;
  agents: string[];
  transports: string[];
  projectScopes: string[];
}

const grouped = computed(() => {
  const map = new Map<string, GroupedServer>();
  for (const e of filtered.value) {
    let g = map.get(e.name);
    if (!g) {
      g = { name: e.name, agents: [], transports: [], projectScopes: [] };
      map.set(e.name, g);
    }
    if (!g.agents.includes(e.agentId)) g.agents.push(e.agentId);
    if (!g.transports.includes(e.transport)) g.transports.push(e.transport);
    if (e.scope !== "global" && !g.projectScopes.includes(e.scope)) g.projectScopes.push(e.scope);
  }
  return [...map.values()].sort((a, b) => a.name.localeCompare(b.name));
});

async function refresh() {
  try {
    await store.fetchAll();
    disabled.value = await listDisabledMcp();
  } catch {
    ElMessage.error(error.value || "读取 MCP 配置失败");
  }
}

/* ---------- 禁用 / 启用 ---------- */

const disabled = ref<DisabledRecord[]>([]);

async function onDisable(entry: McpEntry) {
  try {
    await ElMessageBox.confirm(
      `禁用将从 ${agentName(entry.agentId)} 的配置中移除 ${entry.name}（完整定义会记录在本地禁用清单，随时可一键还原；移除前自动快照）。`,
      "确认禁用",
      { type: "warning", confirmButtonText: "禁用", cancelButtonText: "取消" }
    );
  } catch {
    return;
  }
  try {
    await disableMcp(entry.agentId, entry.name, entry.scope);
    ElMessage.success("已禁用，可在下方禁用清单中还原");
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function onEnable(record: DisabledRecord) {
  try {
    await enableMcp(record.id);
    ElMessage.success(`已还原 ${record.name} 到 ${agentName(record.agentId)}`);
    await refresh();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

/* ---------- 连通性测试 ---------- */

const testingKey = ref("");

function describeResult(r: ConnectivityResult): string {
  if (r.status === "ok") {
    const who = [r.serverName, r.serverVersion].filter(Boolean).join(" v");
    return `连通 ${r.latencyMs}ms${who ? ` · ${who}` : ""}`;
  }
  return r.error ?? "失败";
}

async function onTest(row: McpEntry) {
  const key = `${row.agentId}|${row.name}|${row.scope}`;
  testingKey.value = key;
  try {
    const r = await testMcp(row.agentId, row.name, row.scope);
    if (r.status === "ok") {
      ElMessage.success(`${row.name}：${describeResult(r)}`);
    } else {
      ElMessage.error(`${row.name}：${describeResult(r)}`);
    }
  } catch (e) {
    ElMessage.error(`${row.name}：${String(e)}`);
  } finally {
    testingKey.value = "";
  }
}

async function onTestDef() {
  let def: McpServerDef;
  try {
    def = buildDef();
  } catch (e) {
    ElMessage.error(`配置解析失败：${e instanceof Error ? e.message : String(e)}`);
    return;
  }
  testingKey.value = "@drawer";
  try {
    const r = await testMcpDef(def);
    if (r.status === "ok") {
      ElMessage.success(`测试通过：${describeResult(r)}`);
    } else {
      ElMessage.warning(`测试失败：${describeResult(r)}`);
    }
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    testingKey.value = "";
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
      <el-radio-group v-model="viewMode">
        <el-radio-button value="flat">按 Agent 明细</el-radio-button>
        <el-radio-button value="grouped">覆盖矩阵</el-radio-button>
      </el-radio-group>
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

    <el-table v-if="viewMode === 'grouped'" :data="grouped" v-loading="loading && !loaded" stripe class="mcp-table">
      <el-table-column label="Server" min-width="180">
        <template #default="{ row }">
          <span class="server-name">{{ row.name }}</span>
        </template>
      </el-table-column>
      <el-table-column label="覆盖 Agent" min-width="260">
        <template #default="{ row }">
          <el-tag
            v-for="a in row.agents"
            :key="a"
            size="small"
            effect="plain"
            class="agent-tag"
          >{{ agentName(a) }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="传输" width="120">
        <template #default="{ row }">
          <el-tag
            v-for="t in row.transports"
            :key="t"
            size="small"
            :type="transportTag(t)"
            effect="plain"
            class="agent-tag"
          >{{ t }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="项目级" width="90">
        <template #default="{ row }">
          <el-tag v-if="row.projectScopes.length" size="small" type="warning" effect="plain">×{{ row.projectScopes.length }}</el-tag>
          <span v-else>—</span>
        </template>
      </el-table-column>
    </el-table>

    <el-table v-else :data="filtered" v-loading="loading && !loaded" stripe class="mcp-table">
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
      <el-table-column label="操作" width="350" fixed="right">
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
            :icon="Link"
            :loading="testingKey === `${row.agentId}|${row.name}|${row.scope}`"
            @click="onTest(row)"
          >测试</el-button>
          <el-button
            size="small"
            :icon="Share"
            :disabled="row.scope !== 'global'"
            :title="row.scope !== 'global' ? '项目级条目不支持同步' : ''"
            @click="openMcpSync(row)"
          >同步</el-button>
          <el-button
            size="small"
            type="warning"
            plain
            :icon="SwitchButton"
            :disabled="row.scope !== 'global'"
            @click="onDisable(row)"
          >禁用</el-button>
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

    <div v-if="disabled.length" class="disabled-section">
      <div class="disabled-head">已禁用条目（{{ disabled.length }}）</div>
      <el-table :data="disabled" size="small" stripe>
        <el-table-column prop="name" label="Server" min-width="160" />
        <el-table-column label="Agent" width="130">
          <template #default="{ row }">{{ agentName(row.agentId) }}</template>
        </el-table-column>
        <el-table-column label="禁用时间" width="170">
          <template #default="{ row }">{{ new Date(row.disabledAt).toLocaleString("zh-CN", { hour12: false }) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="110" fixed="right">
          <template #default="{ row }">
            <el-button size="small" type="success" plain :icon="SwitchButton" @click="onEnable(row)">还原</el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <!-- MCP 同步对话框 -->
    <el-dialog v-model="syncVisible" :title="`同步 MCP：${syncSourceEntry?.name ?? ''}（源：${agentName(syncSourceEntry?.agentId ?? '')}）`" width="460px">
      <el-form label-position="top">
        <el-form-item label="目标 Agent" required>
          <el-checkbox-group v-model="syncTargetAgents">
            <el-checkbox
              v-for="a in agents.filter((x) => !syncSourceEntry || x.id !== syncSourceEntry.agentId)"
              :key="a.id"
              :value="a.id"
              :label="a.name"
            />
          </el-checkbox-group>
        </el-form-item>
        <el-alert
          type="info"
          :closable="false"
          title="密钥保护规则"
          description="TOKEN/KEY/SECRET 等密钥类 env：目标已有则保留本地值，没有则写占位符；其余 env 照常同步。写入前自动快照。"
        />
        <template v-if="syncMcpResults">
          <el-divider />
          <div v-for="r in syncMcpResults" :key="r.agentId" class="deploy-line">
            <el-tag size="small" :type="r.ok ? 'success' : 'danger'">{{ agentName(r.agentId) }}</el-tag>
            <span class="sync-err">{{ r.error ?? "完成" }}</span>
          </div>
        </template>
      </el-form>
      <template #footer>
        <el-button @click="syncVisible = false">关闭</el-button>
        <el-button type="primary" :loading="syncingMcp" @click="confirmMcpSync">执行同步</el-button>
      </template>
    </el-dialog>

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
        <el-button class="save-btn" :loading="testingKey === '@drawer'" @click="onTestDef">测试连接</el-button>
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
.disabled-section { margin-top: 20px; }
.disabled-head { font-weight: 600; margin-bottom: 10px; }
.agent-tag { margin-right: 6px; margin-bottom: 2px; }
.sync-err { font-size: 12px; color: var(--el-text-color-secondary); word-break: break-all; }
</style>
