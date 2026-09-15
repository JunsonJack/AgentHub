<div align="center">

# AgentHub

**桌面端 AI Agent 综合管理控制台 —— 把散落在各处的 Skill、MCP 配置和 AI 工具集中起来**

统一查看 · 统一配置 · 跨 Agent 分发 · 本地优先 · 无账号

[![Platform](https://img.shields.io/badge/platform-Windows%2010%2B-blue)](#快速开始)
[![Engine](https://img.shields.io/badge/Tauri-2-orange)](https://tauri.app)
[![Frontend](https://img.shields.io/badge/Vue%203-TypeScript-green)](https://vuejs.org)
[![Tests](https://img.shields.io/badge/tests-56%20passing-brightgreen)](#测试)
[![License](https://img.shields.io/badge/license-MIT-lightgrey)](#license)

</div>

---

## 目录

- [项目背景](#项目背景)
- [核心能力](#核心能力)
- [核心概念](#核心概念)
- [支持的 Agent](#支持的-agent)
- [架构](#架构)
- [快速开始](#快速开始)
- [工程结构](#工程结构)
- [安全设计](#安全设计)
- [测试](#测试)
- [Roadmap](#roadmap)
- [FAQ](#faq)
- [License](#license)

## 项目背景

一台重度使用 AI 的电脑上，通常同时装着多个 Agent：Claude Code、ZCode、Codex CLI、Pi、Cursor、Gemini CLI……它们各自为政：

1. **配置割裂** —— 每个 Agent 的 MCP 配置路径、格式都不一样（JSON / JSONC / TOML），装一个新 MCP server 要挨个改一遍；Skill 目录也是一人一套，同一个 skill 想给多个 Agent 用只能手动复制。
2. **内容散落** —— 好用的 Skill 和 MCP 散落在 GitHub 与各类目录站，看到、试用、沉淀没有统一入口。
3. **工具没有家** —— 日常发现的 AI 工具（网页 / CLI / 桌面应用）缺一个带分类、带笔记的本地收集本。

网页目录类产品（Smithery、mcp.so、PulseMCP 等）只做"浏览和查"，不做"本机管控"；各家 Agent 自带的 marketplace 只服务自家生态。**AgentHub 补的是"本机控制台"这一层**：读写真实配置、连通性实测、跨 Agent 分发与同步。

## 核心能力

### Agent 识别与总览
- 自动扫描本机 Agent 安装位置，仪表盘展示 Skill / MCP 数量与配置健康度
- **健康度检查**：配置解析失败、非法条目（stdio 缺 command 等）、启动命令不在 PATH、skill 缺 SKILL.md
- 支持在设置中手动覆写任意 Agent 的配置路径（应对自定义安装）

### MCP 管理中心
- 跨 Agent 汇总所有 MCP server，**覆盖矩阵**视图按 server 去重、标注装在哪些 Agent 上
- 编辑抽屉双模式：**表单模式**（command / args / env / 传输类型）+ **源码模式**（原始 JSON，未知字段原样保留），写入前 schema 校验
- **一键下发**：把某个 server 安装到一个或多个选中的 Agent
- **禁用 / 一键还原**：禁用时移出配置并完整记录定义，随时恢复
- **连通性测试**：stdio 型实际拉起进程走 `initialize` 握手（拿到 serverInfo / 延迟），http 型健康检查
- **跨 Agent 同步**：以某 Agent 的版本为源传播到其他 Agent；密钥类 env 自动保护（见[安全设计](#安全设计)）

### Skill 管理中心
- 跨 Agent 列表，区分用户级 / 项目级作用域；SKILL.md 渲染（DOMPurify 消毒）
- **收编（adopt）**：把散落在各 Agent 目录的存量 skill 批量拉进中央库，支持 dry-run 预览，同名冲突拒绝覆盖
- 删除走**回收站**，可随时恢复；支持从本地文件夹 / **zip** 导入；**插件包（bundle）** 解析后一键装到多个 Agent
- **跨 Agent 同步**：文件级 diff → 差异预览 → 传播；上游删除的文件默认扣留，只有显式确认才会删除

### 市场与收藏集
- **skills.sh** 匿名搜索与安装（快照下载，无需任何凭证）；**SkillsMP** 搜索（匿名可用，配置 `sk_live_` 密钥提升配额并启用语义排序）
- 粘贴 Git URL 安装：仓库根、`/tree/branch/子目录`、`URL#子路径` 均可（自动 shallow + sparse clone）
- **收藏集**：内置精选清单（随软件版本化）+ 用户自建条目（标签 / 星级 / 备注）；支持导出 / 导入 JSON
- **版本更新**：对 `git:` 与 `skills.sh:` 来源的条目检测上游更新，check 与 apply 严格分离，更新前整体快照

### 配置 Profile
- 把一组 skill（中央库）+ MCP（全局条目）保存为命名组合（如"工作 / 写作"）
- 一键应用到指定 Agent，逐项结果反馈，写入前自动快照

### AI 工具箱
- 工具卡片（名称、链接、标签、星级、使用笔记），刻意做轻——只做"卡片 + 笔记"，不做网页快照
- 粘贴 URL 自动抓取标题与描述

## 核心概念

| 概念 | 含义 |
|---|---|
| **Agent** | 任何可被读写的 AI Agent（CLI / IDE 插件 / 桌面应用） |
| **Connector** | 每个 Agent 一个适配器；路径与格式声明在 `registry.json`（数据而非硬编码） |
| **Library（中央库）** | 所有安装 / 收编内容的单一事实来源；"部署到 Agent"是独立的显式步骤 |
| **Assignment（绑定）** | 对象与 Agent 的安装关系，带作用域（全局 / 项目级） |
| **Collection（收藏集）** | 本地编目条目，带标签、备注、星级 |
| **Snapshot（快照）** | 任何写入前的自动备份，回滚本身也可撤销 |
| **Profile** | skill + MCP 的命名组合，一键应用 |

## 支持的 Agent

> 路径均在本机（Windows）实测核验，详见 [docs/W0-本机调研笔记.md](docs/W0-本机调研笔记.md)。

| Agent | MCP 配置位置 | Skill 目录 | 格式 | 读取 | 写入 |
|---|---|---|---|---|---|
| Claude Code | `~/.claude.json`（含项目级 `projects.*.mcpServers`） | `~/.claude/skills` | JSON | ✅ | ✅ |
| ZCode | `~/.zcode/cli/config.json`（`mcp.servers`） | `~/.zcode/skills`、`~/.agents/skills` | JSON | ✅ | ✅ |
| Codex CLI | `~/.codex/config.toml`（`[mcp_servers.*]`） | `~/.codex/skills` | TOML | ✅ | ✅（toml_edit 保留注释） |
| Pi | `~/.pi/agent/mcp.json`（`mcpServers`） | `~/.pi/agent/skills`、`~/.agents/skills` | JSON | ✅ | ✅ |
| Cursor | `~/.cursor/mcp.json` | `~/.cursor/skills` | JSONC | ✅ 容忍注释 | ✅ 结构保留写入（注释可能丢失，有快照） |
| Gemini CLI | `~/.gemini/settings.json` | — | JSON | ✅ | ✅ |
| Claude Desktop | `%APPDATA%\Claude\claude_desktop_config.json` | — | JSON | ✅ | ✅ |

新 Agent 接入 = 在 `registry.json` 加一条声明 + 在 `connector::generic::make_connector` 挂一个分支；配置格式已是标准 `mcpServers` 映射的（如 Pi、Gemini CLI）直接复用 `GenericJsonMcpConnector`，零新增代码。

> **skillDirs 的顺序有语义**：读取时扫全部目录，**写入（安装 skill / 同步）只认首项**（`registry.rs` 与 `sync.rs` 均取 `dirs.first()`）。所以私有目录必须排在共享目录之前 —— 例如 Pi 写作 `~/.pi/agent/skills, ~/.agents/skills`，避免把 skill 误写进跨 Agent 共享的 `~/.agents/skills` 而连带影响 ZCode。

## 架构

```
┌──────────────────────────────────────────────┐
│                UI（Vue 3 + Element Plus）     │
│  总览 │ MCP 中心 │ Skill 中心 │ 市场与收藏      │
│  配置 Profile │ AI 工具箱 │ 设置               │
└───────────────────┬──────────────────────────┘
                    │ Tauri IPC（21 个命令）
┌───────────────────┴──────────────────────────┐
│        src-tauri（Tauri 壳，仅命令转发）        │
├──────────────────────────────────────────────┤
│        agenthub-core（与框架解耦的核心层）       │
│  ├─ connector/  每 Agent 一个适配器            │
│  ├─ registry    Agent 注册表（数据驱动）        │
│  ├─ market      skills.sh / SkillsMP / Git    │
│  ├─ sync        跨 Agent 同步引擎              │
│  ├─ updater     版本更新（check/apply 分离）    │
│  ├─ library     中央库（文件形态 + manifest）   │
│  ├─ collection  收藏集编目                     │
│  ├─ snapshot    写入前快照 / 回滚              │
│  ├─ trash       回收站（可恢复删除）            │
│  ├─ health      配置健康度                     │
│  ├─ runner      MCP 连通性测试                 │
│  └─ store       SQLite（设置/禁用清单/收藏/Profile）│
└──────────────────────────────────────────────┘
```

核心层不依赖 Tauri，P2 可无痛派生 CLI。

## 快速开始

### 前置要求

| 依赖 | 版本 | 说明 |
|---|---|---|
| Node.js | ≥ 20 | 前端构建 |
| Rust（MSVC） | stable | 含 `cargo`；Windows 需 VS Build Tools（C++ 工作负载） |
| WebView2 | — | Win10/11 一般自带 |
| Git | 任意 | Git URL 安装 skill 时使用（可选） |

### 从安装包

构建产物 `target/release/bundle/nsis/AgentHub_0.1.0_x64-setup.exe`（约 3.8MB），双击安装。

### 从源码运行

```bash
git clone https://github.com/JunsonJack/AgentHub.git
cd AgentHub
npm install        # 前端依赖（esbuild 的 install script 需按 npm 提示放行）
npm run tauri dev  # 开发模式：前端热更 + Rust 增量编译
```

### 打包

```bash
npm run tauri build  # 产物：target/release/bundle/nsis/*-setup.exe
```

### 常用校验命令

```bash
npm run build                            # 前端类型检查（vue-tsc）+ 构建
cargo check --workspace --all-targets    # Rust 全工作区
cargo test -p agenthub-core              # 核心层测试
cargo run -p agenthub-core --example list_agents      # 冒烟：读本机 Agent 真实状态
cargo run -p agenthub-core --example market_smoke     # 冒烟：市场搜索与安装（写入临时目录）
cargo run -p agenthub-core --example mcp_test_smoke   # 冒烟：对本机 MCP server 逐个握手
```

## 工程结构

```
AgentHub/
├── src/                        # 前端
│   ├── views/                  # 7 个页面
│   ├── stores/                 # Pinia
│   ├── api/                    # Tauri IPC 封装 + 类型（与 Rust 模型一一对应）
│   └── utils/markdown.ts       # SKILL.md 渲染（marked + DOMPurify）
├── src-tauri/                  # Tauri 壳：tauri.conf / capabilities / IPC 命令转发
├── crates/agenthub-core/       # 核心层（见架构图），不依赖任何 UI 框架
│   ├── src/registry.json       # Agent 注册表：路径/格式声明是数据
│   ├── src/curated.json        # 收藏集内置精选清单
│   ├── tests/                  # 14 个测试套件（56 例）
│   └── examples/               # 真机冒烟脚本
├── docs/                       # W0 调研笔记等
└── scripts/                    # 图标生成等辅助脚本
```

## 安全设计

写坏用户配置是这类工具的信任崩塌点，AgentHub 用四道机制兜底：

| 机制 | 说明 |
|---|---|
| **快照前置** | 任何写配置动作前自动备份到 `%APPDATA%\AgentHub\snapshots\`；回滚前对当前文件再快照一次，回滚本身可撤销 |
| **格式保留写入** | Cursor 的 JSONC、Codex 的 TOML 绝不 parse 后整体重写——JSON 保键序，TOML 走 `toml_edit` 保留注释与排版（有回归测试固化） |
| **绝不代删** | 同名冲突一律拒绝覆盖；skill 删除先进回收站；同步时上游删除的文件默认扣留，只有显式确认才会删除 |
| **密钥不外带** | 跨 Agent 传播 MCP 定义时，`TOKEN/KEY/SECRET/PASSWORD/CREDENTIAL` 类 env：目标已有则保留本地值，缺失则写占位符；其余 env 照常同步 |
| **密钥本机加密** | 设置页密钥库用 **Windows DPAPI** 加密（绑定当前用户）；列表只显示脱敏值，密文不回传前端 |

其他约定：密钥库走 Windows DPAPI（绑定当前用户）；SkillsMP 密钥只存本机 SQLite；市场下载内容渲染前经 DOMPurify 消毒；WebView 默认启用 CSP；市场 HTTP 自动读取 `HTTPS_PROXY` 等环境变量，失败自动降级直连。

## 测试

```bash
cargo test -p agenthub-core   # 14 个套件 / 56 例
```

覆盖：格式保留写入（TOML 注释无损、JSON 键序不变）、快照与回滚、收编 dry-run 与冲突拒绝、MCP 禁用/还原往返、健康度判定、市场 JSON 解析与 Git URL 解析、同步差异与删除扣留、Profile 密钥保护、连通性握手（Python 假服务器确定性用例）。写路径测试全部走临时目录，不触碰真实用户数据。

## Roadmap

- [x] **P0** —— Agent 识别、MCP/Skill 双中心、快照安全网、健康度
- [x] **P1** —— 同步引擎、收藏集、连通性测试、配置 Profile
- [x] **P2（部分提前）** —— AI 工具箱、Skill 版本与更新
- [x] **P2（安全与硬缺口）** —— 密钥 DPAPI 加密 + 设置页密钥库、Cursor JSONC 写入、项目级 MCP 编辑（Claude Code）、CSP 收紧
- [ ] **P2（按反馈排期）** —— 插件包支持（前端入口）、反向管理、CLI 派生、自动更新、zip 导入、书签导入、Git 备份

## FAQ

**Q：SkillsMP / skills.sh 需要登录吗？**
skills.sh 的搜索与安装走匿名端点，无需任何凭证；SkillsMP 匿名每天 50 次搜索，在设置页配置 `sk_live_` 密钥后每天 500 次并启用语义排序。密钥只存本机。

**Q：Cursor 的 MCP 能读写吗？**
能。`~/.cursor/mcp.json` 支持 JSONC（带注释）读取与写入。整体重写时注释可能无法原样保留，写入前会自动快照。

**Q：同步 / Profile 会把我的 API Key 复制到别的 Agent 吗？**
不会。密钥类 env 在目标 Agent 已有配置时保留其本地值；目标没有时写入 `<请在目标 Agent 中填写>` 占位符。非密钥 env 照常同步。

**Q：删错了能找回吗？**
能。Skill 删除进入回收站（设置页可一键恢复）；每次配置写入前都有快照，回滚前还会对当前文件再快照一次。

**Q：为什么我的 Gemini CLI / Claude Desktop 显示"未检测到"？**
本机未安装即不显示数据；注册表已声明其默认路径，装上后重启应用即可识别。

## License

MIT
