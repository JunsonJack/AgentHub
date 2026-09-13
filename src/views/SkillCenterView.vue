<script setup lang="ts">
import { Files } from "@element-plus/icons-vue";
// P0 第 3 项：Skill 列表 / 详情抽屉 / 导入 / 收编（adopt）
// Connector v0 已在注册表中声明各 Agent 的 skillDirs，读取能力在下一步接入。
</script>

<template>
  <el-empty description="Skill 管理中心 —— P0 第 3 项，脚手架阶段占位">
    <template #image>
      <el-icon :size="64" color="var(--el-color-primary-light-5)"><Files /></el-icon>
    </template>
    <div class="plan-hint">
      <p>规划能力：跨 Agent skill 列表（用户级 / 项目级）、SKILL.md 渲染、从文件夹 / zip / GitHub 导入、</p>
      <p>存量 skill 收编（adopt，支持 dry-run）、启用 / 禁用 / 删除。</p>
    </div>
  </el-empty>
</template>

<style scoped>
.plan-hint { color: var(--el-text-color-secondary); font-size: 13px; line-height: 1.8; }
</style>
