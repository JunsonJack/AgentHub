import { createRouter, createWebHashHistory } from "vue-router";

// Tauri 文件协议下必须用 hash history
const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "dashboard", component: () => import("../views/DashboardView.vue"), meta: { title: "总览" } },
    { path: "/mcp", name: "mcp", component: () => import("../views/McpCenterView.vue"), meta: { title: "MCP 管理中心" } },
    { path: "/skills", name: "skills", component: () => import("../views/SkillCenterView.vue"), meta: { title: "Skill 管理中心" } },
    { path: "/collection", name: "collection", component: () => import("../views/CollectionView.vue"), meta: { title: "市场与收藏" } },
    { path: "/toolbox", name: "toolbox", component: () => import("../views/ToolboxView.vue"), meta: { title: "AI 工具箱" } },
    { path: "/settings", name: "settings", component: () => import("../views/SettingsView.vue"), meta: { title: "设置" } },
  ],
});

export default router;
