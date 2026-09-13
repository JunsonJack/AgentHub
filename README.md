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

## 当前状态（P0 主体完成）

- [x] 前端 6 页面骨架 + 布局 + 路由 + 状态管理
- [x] Rust 工作区：`src-tauri`（Tauri 壳）与 `agenthub-core`（核心层）分离
- [x] Connector v0：注册表数据化（6 个 Agent 声明）+ Claude Code / ZCode / Codex / Claude Desktop / Gemini 的 MCP 读取 + 写入（快照前置、键序保留、TOML 注释保留）
- [x] MCP 中心：编辑抽屉（表单 / 源码双模式）、schema 校验、多 Agent 批量下发、删除
- [x] Skill 中心：跨 Agent 列表（用户级 / 项目级）、SKILL.md 详情、中央库、收编 adopt（dry-run 预览 + 冲突拒绝）
- [x] 快照：浏览 / 按路径过滤 / 一键回滚（回滚前再快照，可撤销）
- [x] 格式保留写入回归测试 + 收编 / 回滚行为测试（10 例）
- [ ] 下一步（P0 收尾）：SKILL.md Markdown 渲染、多行 YAML frontmatter 解析、配置健康度校验、异常处理打磨

## 设计红线

1. 一切写配置动作前必须自动快照。
2. 格式保留写入：Cursor 的 JSONC、Codex 的 TOML，绝不能 parse 后整体重写。
3. 密钥不跨 Agent 同步；展示时脱敏。
4. core 层不依赖 Tauri。
