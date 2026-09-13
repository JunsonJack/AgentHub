use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("io 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("json 错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("toml 解析错误: {0}")]
    TomlParse(String),

    #[error("sqlite 错误: {0}")]
    Sql(#[from] rusqlite::Error),

    #[error("不支持的操作: {0}")]
    Unsupported(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}
