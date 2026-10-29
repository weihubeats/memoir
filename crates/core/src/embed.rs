use std::sync::Arc;

use anyhow::Context as _;
use async_trait::async_trait;
use serde_json::json;

use crate::Ctx;

pub const EMBED_BASE_ENV: &str = "MEMOIR_EMBED_BASE";
pub const EMBED_MODEL_ENV: &str = "MEMOIR_EMBED_MODEL";

/// 嵌入模型抽象。M2 用 Ollama 本地 HTTP（嵌入计算不出本机）；不配置即纯关键词检索。
#[async_trait]
pub trait Embedder: Send + Sync {
    fn name(&self) -> &str;
    async fn embed(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>>;
}

/// 设了 `MEMOIR_EMBED_MODEL` 才启用（默认指向本机 Ollama，`MEMOIR_EMBED_BASE` 可覆盖）。
pub fn from_env() -> Option<Arc<dyn Embedder>> {
    let model = std::env::var(EMBED_MODEL_ENV)
        .ok()
        .filter(|s| !s.is_empty())?;
    let base =
        std::env::var(EMBED_BASE_ENV).unwrap_or_else(|_| "http://127.0.0.1:11434".to_owned());
    Some(Arc::new(OllamaEmbedder {
        base,
        model,
        client: reqwest::Client::new(),
    }))
}

pub struct OllamaEmbedder {
    base: String,
    model: String,
    client: reqwest::Client,
}

#[async_trait]
impl Embedder for OllamaEmbedder {
    fn name(&self) -> &str {
        &self.model
    }

    async fn embed(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        let url = format!("{}/api/embed", self.base);
        let resp = self
            .client
            .post(url)
            .json(&json!({ "model": self.model, "input": texts }))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("Ollama embed 请求失败：{status}");
        }
        let body: serde_json::Value = resp.json().await?;
        let embeddings = body
            .get("embeddings")
            .and_then(|v| v.as_array())
            .context("Ollama 响应缺少 embeddings 字段")?;
        embeddings
            .iter()
            .map(|e| {
                e.as_array()
                    .context("embedding 不是数组")?
                    .iter()
                    .map(|x| Ok(x.as_f64().context("embedding 元素不是数字")? as f32))
                    .collect()
            })
            .collect()
    }
}

pub fn vector_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn blob_to_vector(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0f64;
    let mut na = 0f64;
    let mut nb = 0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += (*x as f64) * (*y as f64);
        na += (*x as f64).powi(2);
        nb += (*y as f64).powi(2);
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// 为一条记录写 embedding（best-effort：嵌入失败不影响记录本身）。
pub async fn store_entry_embedding(ctx: &Ctx, entry_id: i64, title: &str, body: &str) {
    let Some(embedder) = &ctx.embedder else {
        return;
    };
    let text = format!("{title}\n{body}");
    if let Ok(vecs) = embedder.embed(vec![text]).await {
        if let Some(v) = vecs.first() {
            let _ = sqlx::query(
                "INSERT INTO embeddings (entry_id, model, dim, vector) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(entry_id) DO UPDATE SET model = ?2, dim = ?3, vector = ?4",
            )
            .bind(entry_id)
            .bind(embedder.name())
            .bind(v.len() as i64)
            .bind(vector_to_blob(v))
            .execute(&ctx.pool)
            .await;
        }
    }
}

/// 为存量记录补 embedding；换模型时重建（个人库量级，全量跑也很快）。
pub async fn backfill(ctx: &Ctx, batch: usize) -> anyhow::Result<usize> {
    let Some(embedder) = &ctx.embedder else {
        anyhow::bail!("未配置嵌入模型（设置 MEMOIR_EMBED_MODEL 后重试）")
    };
    let model = embedder.name().to_owned();
    sqlx::query("DELETE FROM embeddings WHERE model != ?1")
        .bind(&model)
        .execute(&ctx.pool)
        .await?;
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM entries WHERE id NOT IN (SELECT entry_id FROM embeddings) ORDER BY id",
    )
    .fetch_all(&ctx.pool)
    .await?;
    let mut done = 0;
    for chunk in ids.chunks(batch.max(1)) {
        let mut texts = Vec::with_capacity(chunk.len());
        for id in chunk {
            let (title, body): (String, String) =
                sqlx::query_as("SELECT title, body FROM entries WHERE id = ?1")
                    .bind(id)
                    .fetch_one(&ctx.pool)
                    .await?;
            texts.push(format!("{title}\n{body}"));
        }
        let vecs = embedder.embed(texts).await?;
        for (id, v) in chunk.iter().zip(&vecs) {
            sqlx::query(
                "INSERT INTO embeddings (entry_id, model, dim, vector) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(entry_id) DO UPDATE SET model = ?2, dim = ?3, vector = ?4",
            )
            .bind(id)
            .bind(&model)
            .bind(v.len() as i64)
            .bind(vector_to_blob(v))
            .execute(&ctx.pool)
            .await?;
            done += 1;
        }
    }
    Ok(done)
}

/// 查询向量：按余弦相似度降序的前 top_k 条 (entry_id, similarity)。
pub async fn search_vectors(
    ctx: &Ctx,
    query: &str,
    top_k: usize,
) -> anyhow::Result<Vec<(i64, f64)>> {
    let Some(embedder) = &ctx.embedder else {
        return Ok(vec![]);
    };
    let qv = embedder
        .embed(vec![query.to_owned()])
        .await?
        .into_iter()
        .next()
        .context("空查询向量")?;
    let rows: Vec<(i64, String, Vec<u8>)> =
        sqlx::query_as("SELECT entry_id, model, vector FROM embeddings")
            .fetch_all(&ctx.pool)
            .await?;
    let mut hits: Vec<(i64, f64)> = rows
        .into_iter()
        .filter(|(_, model, _)| model == embedder.name())
        .map(|(id, _, blob)| (id, cosine(&qv, &blob_to_vector(&blob))))
        .filter(|(_, sim)| *sim > 0.05)
        .collect();
    hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    hits.truncate(top_k);
    Ok(hits)
}
