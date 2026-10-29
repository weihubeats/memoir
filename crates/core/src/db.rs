use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::repo;

pub const DB_ENV_VAR: &str = "MEMOIR_DB";

/// 库文件路径：环境变量 `MEMOIR_DB` 优先，否则落到系统数据目录。
pub fn default_db_path() -> anyhow::Result<PathBuf> {
    if let Ok(p) = std::env::var(DB_ENV_VAR) {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("无法确定系统数据目录"))?;
    Ok(base.join("memoir").join("memoir.db"))
}

pub async fn open_pool(path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(opts)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// 打开库并确保迁移、种子与回收站清理就绪（幂等），CLI 与 MCP server 的统一入口。
pub async fn ensure_pool(path: &Path) -> anyhow::Result<SqlitePool> {
    let pool = open_pool(path).await?;
    repo::seed_self_entity(&pool).await?;
    repo::auto_purge(&pool).await?;
    Ok(pool)
}
