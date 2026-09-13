<script setup lang="ts">
// P0 第 1 项的一部分：Agent 配置路径覆写；P0 第 4 项：快照策略与历史回滚
</script>

<template>
  <el-empty description="设置 —— 脚手架阶段占位">
    <div class="plan-hint">
      <p>规划能力：Agent 配置路径手动覆写、快照策略与历史回滚浏览、主题、数据目录。</p>
      <p>数据目录（当前默认）：%APPDATA%\AgentHub —— 快照与 SQLite 库都存放在这里。</p>
    </div>
  </el-empty>
</template>

<style scoped>
.plan-hint { color: var(--el-text-color-secondary); font-size: 13px; line-height: 1.8; }
</style>
