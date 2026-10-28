use std::path::Path;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{db, embed::Embedder};

/// 进程内共享的核心上下文：连接池 + 可选嵌入器。
/// CLI / MCP server / 未来桌面端统一持有它，保证各入口行为一致。
#[derive(Clone)]
pub struct Ctx {
    pub pool: SqlitePool,
    pub embedder: Option<Arc<dyn Embedder>>,
}

impl Ctx {
    /// 打开库（迁移 + 种子）并装配上下文。
    pub async fn open(path: &Path, embedder: Option<Arc<dyn Embedder>>) -> anyhow::Result<Self> {
        let pool = db::ensure_pool(path).await?;
        Ok(Self { pool, embedder })
    }

    /// 测试用：手动组装（自选嵌入器/内存库）。
    pub fn from_parts(pool: SqlitePool, embedder: Option<Arc<dyn Embedder>>) -> Self {
        Self { pool, embedder }
    }
}
