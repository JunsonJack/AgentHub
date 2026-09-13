use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::error::Result;
use crate::util::app_data_dir;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS snapshots (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT,
    source_path TEXT NOT NULL,
    backup_path TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS collection_items (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT NOT NULL,             -- skill | mcp | tool
    name       TEXT NOT NULL,
    source     TEXT,                      -- GitHub / URL / 手工
    tags       TEXT,                      -- JSON 数组
    note       TEXT,
    stars      INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_collection_kind ON collection_items(kind);

CREATE TABLE IF NOT EXISTS disabled_mcp (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT NOT NULL,
    name        TEXT NOT NULL,
    scope       TEXT NOT NULL,
    def_json    TEXT NOT NULL,
    disabled_at INTEGER NOT NULL
);
"#;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(&app_data_dir().join("agenthub.db"))
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// 键值设置（SkillsMP API 密钥等本地配置；P2 换 OS keychain）
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .map_err(Into::into)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }

    /* ---------- MCP 禁用清单（移出配置 + 记录，启用时还原） ---------- */

    pub fn add_disabled_mcp(
        &self,
        agent_id: &str,
        name: &str,
        scope: &str,
        def_json: &serde_json::Value,
    ) -> Result<i64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO disabled_mcp(agent_id, name, scope, def_json, disabled_at)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![agent_id, name, scope, def_json.to_string(), now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_disabled_mcp(&self) -> Result<Vec<DisabledRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, agent_id, name, scope, def_json, disabled_at FROM disabled_mcp ORDER BY disabled_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(DisabledRecord {
                id: r.get(0)?,
                agent_id: r.get(1)?,
                name: r.get(2)?,
                scope: r.get(3)?,
                def: serde_json::from_str(&r.get::<_, String>(4)?)
                    .unwrap_or(serde_json::Value::Null),
                disabled_at: r.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// 取出并删除一条禁用记录（启用时还原用）
    pub fn take_disabled_mcp(&self, id: i64) -> Result<Option<DisabledRecord>> {
        let all = self.list_disabled_mcp()?;
        let found = match all.into_iter().find(|r| r.id == id) {
            Some(r) => r,
            None => return Ok(None),
        };
        self.conn
            .execute("DELETE FROM disabled_mcp WHERE id = ?1", [id])?;
        Ok(Some(found))
    }
}

/// 已禁用 MCP 条目的记录
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledRecord {
    pub id: i64,
    pub agent_id: String,
    pub name: String,
    pub scope: String,
    /// 原始定义（含未知字段），启用时原样还原
    pub def: serde_json::Value,
    pub disabled_at: i64,
}
