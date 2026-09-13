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

CREATE TABLE IF NOT EXISTS profiles (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS profile_items (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    ref_name   TEXT NOT NULL,
    def_json   TEXT NOT NULL DEFAULT '{}'
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

    /* ---------- 收藏集用户条目 ---------- */

    pub fn add_collection_item(
        &self,
        kind: &str,
        name: &str,
        source: &str,
        tags_json: &str,
        note: &str,
        stars: i64,
    ) -> Result<i64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO collection_items(kind, name, source, tags, note, stars, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![kind, name, source, tags_json, note, stars, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_collection_items(
        &self,
    ) -> Result<Vec<(i64, String, String, String, String, String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, source, tags, note, stars FROM collection_items ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get::<_, Option<String>>(4)?.unwrap_or_else(|| "[]".into()),
                r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                r.get(6)?,
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn update_collection_item(
        &self,
        id: i64,
        name: &str,
        source: &str,
        tags_json: &str,
        note: &str,
        stars: i64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE collection_items SET name=?2, source=?3, tags=?4, note=?5, stars=?6 WHERE id=?1",
            rusqlite::params![id, name, source, tags_json, note, stars],
        )?;
        Ok(())
    }

    pub fn delete_collection_item(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM collection_items WHERE id=?1", [id])?;
        Ok(())
    }

    /* ---------- 配置 Profile ---------- */

    pub fn add_profile(&self, name: &str) -> Result<i64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO profiles(name, created_at) VALUES(?1, ?2)",
            rusqlite::params![name, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_profiles(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM profiles ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_profile(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM profile_items WHERE profile_id=?1", [id])?;
        self.conn.execute("DELETE FROM profiles WHERE id=?1", [id])?;
        Ok(())
    }

    /// 追加一条 profile 项：kind = skill（ref_name=中央库名）| mcp（ref_name=server 名，def_json=完整定义）
    pub fn add_profile_item(
        &self,
        profile_id: i64,
        kind: &str,
        ref_name: &str,
        def_json: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO profile_items(profile_id, kind, ref_name, def_json) VALUES(?1, ?2, ?3, ?4)",
            rusqlite::params![profile_id, kind, ref_name, def_json],
        )?;
        Ok(())
    }

    pub fn list_profile_items(
        &self,
        profile_id: i64,
    ) -> Result<Vec<(i64, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, ref_name, def_json FROM profile_items WHERE profile_id=?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([profile_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_profile_item(&self, item_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM profile_items WHERE id=?1", [item_id])?;
        Ok(())
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
