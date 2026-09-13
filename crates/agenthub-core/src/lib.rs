//! AgentHub 核心层：与 UI 框架（Tauri）完全解耦，CLI / 桌面端共享。
//!
//! 分层：
//! - `model`     —— 数据模型（serde camelCase，与前端 TS 类型一一对应）
//! - `connector` —— 每 Agent 一个适配器；路径与格式声明在 `registry.json`（数据而非硬编码）
//! - `registry`  —— 注册表装载与跨 Agent 汇总
//! - `snapshot`  —— 任何写配置前的自动备份（安全网）
//! - `store`     —— SQLite（条目、绑定、快照、收藏元数据）

pub mod connector;
pub mod error;
pub mod health;
pub mod library;
pub mod market;
pub mod model;
pub mod registry;
pub mod snapshot;
pub mod store;
pub mod util;

pub use error::{CoreError, Result};
