//! 收藏集（P1 第 6 项）：本地编目数据库。
//! 内置精选清单随软件版本化（curated.json，条目均经真机连通性/存在性验证）；
//! 用户条目存 SQLite collection_items。

use crate::store::Store;
use serde::{Deserialize, Serialize};

pub const CURATED_JSON: &str = include_str!("curated.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionEntry {
    /// 用户条目为 SQLite id；内置条目为 None
    pub id: Option<i64>,
    /// skill | mcp | tool
    pub kind: String,
    pub name: String,
    /// 安装引用：`skills.sh:<owner/repo/slug>`、`git:<url>`、`library:<name>` 或说明性文字
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub stars: i64,
    /// 内置精选不可编辑/删除
    #[serde(default)]
    pub built_in: bool,
}

#[derive(Deserialize)]
struct CuratedFile {
    #[serde(default)]
    items: Vec<CuratedItem>,
}

#[derive(Deserialize)]
struct CuratedItem {
    name: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    note: String,
}

pub fn list_curated() -> Vec<CollectionEntry> {
    let file: CuratedFile = match serde_json::from_str(CURATED_JSON) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    file.items
        .into_iter()
        .map(|i| CollectionEntry {
            id: None,
            kind: "skill".into(),
            name: i.name,
            source: i.source,
            tags: i.tags,
            note: i.note,
            stars: 0,
            built_in: true,
        })
        .collect()
}

/* ---------- 用户条目 CRUD（SQLite） ---------- */

fn row_to_entry(id: i64, kind: String, name: String, source: String, tags: String, note: String, stars: i64) -> CollectionEntry {
    let tags: Vec<String> = serde_json::from_str(&tags).unwrap_or_default();
    CollectionEntry {
        id: Some(id),
        kind,
        name,
        source,
        tags,
        note,
        stars,
        built_in: false,
    }
}

pub fn add_item(store: &Store, entry: &CollectionEntry) -> crate::error::Result<i64> {
    store.add_collection_item(
        &entry.kind,
        &entry.name,
        &entry.source,
        &serde_json::to_string(&entry.tags)?,
        &entry.note,
        entry.stars,
    )
}

pub fn list_items(store: &Store) -> crate::error::Result<Vec<CollectionEntry>> {
    Ok(store
        .list_collection_items()?
        .into_iter()
        .map(|(id, kind, name, source, tags, note, stars)| {
            row_to_entry(id, kind, name, source, tags, note, stars)
        })
        .collect())
}

pub fn update_item(store: &Store, entry: &CollectionEntry) -> crate::error::Result<()> {
    store.update_collection_item(
        entry.id.ok_or(crate::error::CoreError::Other("缺少条目 id".into()))?,
        &entry.name,
        &entry.source,
        &serde_json::to_string(&entry.tags)?,
        &entry.note,
        entry.stars,
    )
}

pub fn delete_item(store: &Store, id: i64) -> crate::error::Result<()> {
    store.delete_collection_item(id)
}
