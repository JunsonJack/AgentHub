# AgentHub

> 桌面端"AI Agent 控制台"：把散落在各 Agent 的 Skill、MCP 配置和好用的 AI 工具集中起来，统一查看、配置、分发和收藏。
>
> 产品规划见 `../AgentHub-产品规划.md`（文档目录）或 [docs/W0-本机调研笔记.md](docs/W0-本机调研笔记.md)。

## 技术栈

Tauri 2 + Vue 3 + TypeScript + Pinia + Element Plus + SQLite（rusqlite）。

## 工程结构

```
AgentHub/
├── src/                        # 前端（Vue 3）
│   ├── views/                  # 6 个页面：总览 / MCP 中心 / Skill 中心 / 收藏集 / 工具箱 / 设置
│   ├── stores/                 # Pinia
│   └── api/                    # Tauri IPC 封装 + 类型（与 Rust 模型一一对应）
├── src-tauri/                  # Tauri 应用层（仅 IPC commands，不含业务逻辑）
├── crates/
│   └── agenthub-core/          # 与框架解耦的核心层（CLI / 桌面共享，P2 可无痛出 CLI）
│       ├── src/registry.json   # Agent 注册表（路径 / 格式声明是数据而非硬编码）
│       ├── src/connector/      # 每 Agent 一个适配器（trait + claude-code/zcode/codex/cursor/...）
│       ├── src/snapshot.rs     # 写配置前自动快照（安全网）
│       └── src/store.rs        # SQLite（条目 / 绑定 / 快照 / 收藏元数据）
├── docs/                       # W0 调研笔记等
└── scripts/                    # 图标生成等辅助脚本
```

## 本地开发

前置：Node 18+、Rust stable（MSVC）、WebView2（Win10/11 自带）。

```bash
npm install          # 前端依赖
npm run tauri dev    # 开发模式（前端热更 + Rust 增量编译）
npm run tauri build  # 打 NSIS 安装包
```

仅验证编译：

```bash
npm run build                              # 前端类型检查 + 构建
cargo check --workspace --tests            # Rust 全工作区
cargo test -p agenthub-core                # 核心层测试（含格式保留写入回归）
```

## 当前状态（P0 功能已全部落地）

- [x] 前端 6 页面（总览 / MCP 中心 / Skill 中心 / 市场与收藏 / AI 工具箱占位 / 设置）+ 路由 + 状态管理
- [x] Rust 工作区：`src-tauri`（Tauri 壳）与 `agenthub-core`（核心层）分离
- [x] Agent 识别与总览：仪表盘（Skill/MCP 数量 + 健康度卡片 + 待处理提醒）
- [x] **配置健康度**：解析失败 / 非法条目 / 启动命令不在 PATH / skill 缺 SKILL.md
- [x] MCP 中心：读取（含项目级作用域）、编辑抽屉（表单/源码双模式）、schema 校验、多 Agent 批量下发、删除、**禁用/一键还原**（移出配置+本地记录）、**覆盖矩阵**视图
- [x] Skill 中心：跨 Agent 列表（用户级/项目级）、SKILL.md 渲染（DOMPurify 消毒）、**收编 adopt（dry-run）**、**删除（回收站可恢复）**、**文件夹导入**、中央库一键安装到任意 Agent
- [x] **市场**：skills.sh 匿名搜索与快照安装、SkillsMP 搜索（密钥可选，语义排序）、Git URL 安装（sparse clone）
- [x] **MCP 连通性测试**（P1 第 7 项提前落地）：stdio 实际拉起进程握手（拿到 serverInfo/延迟）、http 健康检查；行内测试 + 编辑抽屉测试
- [x] 安全网：一切写配置前自动快照；**快照浏览/按路径过滤/一键回滚（回滚前再快照）**；手动清理旧快照
- [x] 设置：SkillsMP 密钥管理、**Agent 路径覆写**、数据目录说明
- [x] **P1 全部提前落地**：
  - 同步引擎（P1-5）：skill 文件级 diff → 差异预览 → 传播，删除差异默认扣留；同步前目标目录整体快照；MCP 传播密钥保护（TOKEN/KEY/SECRET 类目标已有保留本地、缺失填占位符）
  - 收藏集（P1-6）：内置精选清单（curated.json 版本化）+ 用户条目 CRUD（标签/星级/备注）+ 来源引用一键安装
  - 配置 Profile（P1-8）：skill+MCP 命名组合，一键应用到多 Agent，逐项结果反馈
- [x] 格式保留写入（TOML 注释/排版、JSON 键序）回归测试；共 40 例核心测试
- [ ] 已知留待项：Codex 被禁条目的自有注释不随启用还原（无关内容零改动，行为有测试固化）；MCP 连通性测试（P1 第 7 项）；skill 启用/禁用（各 Agent 无统一机制，暂不做有歧义的伪禁用）

## 设计红线

1. 一切写配置动作前必须自动快照。
2. 格式保留写入：Cursor 的 JSONC、Codex 的 TOML，绝不能 parse 后整体重写。
3. 密钥不跨 Agent 同步；展示时脱敏。
4. core 层不依赖 Tauri。
