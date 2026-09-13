use std::path::Path;

use rusqlite::Connection;

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
}
