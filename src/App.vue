<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { Odometer, Connection, Files, Star, Box, MagicStick, Setting } from "@element-plus/icons-vue";

const route = useRoute();
const title = computed(() => (route.meta.title as string) ?? "AgentHub");
</script>

<template>
  <el-container class="app-shell">
    <el-aside width="220px" class="app-aside">
      <div class="brand">
        <div class="brand-logo">A</div>
        <div>
          <div class="brand-name">AgentHub</div>
          <div class="brand-sub">本地 AI Agent 控制台</div>
        </div>
      </div>
      <el-menu :default-active="route.path" router class="app-menu">
        <el-menu-item index="/"><el-icon><Odometer /></el-icon><span>总览</span></el-menu-item>
        <el-menu-item index="/mcp"><el-icon><Connection /></el-icon><span>MCP 管理中心</span></el-menu-item>
        <el-menu-item index="/skills"><el-icon><Files /></el-icon><span>Skill 管理中心</span></el-menu-item>
        <el-menu-item index="/collection"><el-icon><Star /></el-icon><span>市场与收藏</span></el-menu-item>
        <el-menu-item index="/toolbox"><el-icon><Box /></el-icon><span>AI 工具箱</span></el-menu-item>
        <el-menu-item index="/profiles"><el-icon><MagicStick /></el-icon><span>配置 Profile</span></el-menu-item>
        <el-menu-item index="/settings"><el-icon><Setting /></el-icon><span>设置</span></el-menu-item>
      </el-menu>
    </el-aside>
    <el-container>
      <el-header class="app-header">
        <span class="header-title">{{ title }}</span>
        <el-tag type="info" effect="plain" size="small">本地优先 · 无账号 · P0 脚手架</el-tag>
      </el-header>
      <el-main class="app-main"><router-view /></el-main>
    </el-container>
  </el-container>
</template>

<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body, #app { height: 100%; }
body { font-family: "Segoe UI", "Microsoft YaHei", sans-serif; }

.app-shell { height: 100%; background: #f5f5f7; }
.app-aside {
  border-right: 1px solid var(--el-border-color-light);
  background: rgba(255, 255, 255, .78);
  backdrop-filter: blur(20px);
  display: flex;
  flex-direction: column;
}
.brand { display: flex; align-items: center; gap: 10px; padding: 18px 16px; }
.brand-logo {
  width: 36px; height: 36px; border-radius: 9px;
  background: #2f6fed; color: #fff;
  display: flex; align-items: center; justify-content: center;
  font-weight: 700; font-size: 18px;
}
.brand-name { font-weight: 700; font-size: 16px; }
.brand-sub { font-size: 11px; color: var(--el-text-color-secondary); }
.app-menu { border-right: none; flex: 1; padding: 8px; background: transparent; }
.app-menu .el-menu-item { border-radius: 9px; margin: 2px 0; }
.app-menu .el-menu-item.is-active { background: #e5f2ff; color: #007aff; font-weight: 600; }

.app-header {
  height: 64px;
  display: flex; align-items: center; justify-content: space-between;
  border-bottom: 1px solid var(--el-border-color-light);
  background: rgba(255,255,255,.72);
  backdrop-filter: blur(20px);
}
.header-title { font-size: 17px; font-weight: 650; letter-spacing: -.01em; }
.app-main { background: var(--el-fill-color-extra-light); overflow: auto; padding: 24px; }
</style>
