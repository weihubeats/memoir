use std::collections::HashMap;

use crate::model::Entry;
use crate::{embed, repo, Ctx};

pub const DEFAULT_TOP_K: i64 = 8;
pub const DEFAULT_MAX_CHARS: usize = 4000;

fn fts_phrase(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

/// 混合检索：FTS5 trigram（≥3 字符）/ LIKE（<3 字符）+ 可选向量，RRF 融合，
/// 平手时新记录优先；结果按字符预算裁剪（≈ token 预算）。
pub async fn search_entries(
    ctx: &Ctx,
    query: &str,
    top_k: i64,
    max_chars: usize,
) -> anyhow::Result<Vec<Entry>> {
    let top_k = top_k.clamp(1, 50);
    let candidate = (top_k * 3) as usize;
    let mut rrf: HashMap<i64, f64> = HashMap::new();

    // 关键词通道
    let kw: Vec<i64> = if query.chars().count() >= 3 {
        sqlx::query_scalar(
            "SELECT e.id FROM entries_fts f JOIN entries e ON e.id = f.rowid \
             WHERE entries_fts MATCH ?1 ORDER BY bm25(entries_fts), e.id LIMIT ?2",
        )
        .bind(fts_phrase(query))
        .bind(candidate as i64)
        .fetch_all(&ctx.pool)
        .await?
    } else {
        let pattern = format!("%{}%", query.replace(['%', '_'], ""));
        sqlx::query_scalar(
            "SELECT id FROM entries WHERE title LIKE ?1 OR body LIKE ?1 \
             ORDER BY occurred_at DESC, id DESC LIMIT ?2",
        )
        .bind(pattern)
        .bind(candidate as i64)
        .fetch_all(&ctx.pool)
        .await?
    };
    for (rank, id) in kw.iter().enumerate() {
        *rrf.entry(*id).or_default() += 1.0 / (60.0 + rank as f64);
    }

    // 向量通道（未配置嵌入器时为空）
    if let Ok(vec_hits) = embed::search_vectors(ctx, query, candidate).await {
        for (rank, (id, _sim)) in vec_hits.iter().enumerate() {
            *rrf.entry(*id).or_default() += 1.0 / (60.0 + rank as f64);
        }
    }

    if rrf.is_empty() {
        return Ok(vec![]);
    }

    // 时间新近微加权（30 天半衰期），仅用于平手时的偏向
    let ids_json = serde_json::to_string(&rrf.keys().collect::<Vec<_>>())?;
    let times: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, occurred_at FROM entries WHERE id IN (SELECT value FROM json_each(?1))",
    )
    .bind(&ids_json)
    .fetch_all(&ctx.pool)
    .await?;
    let now = chrono::Utc::now();
    for (id, occurred) in times {
        let age_days = chrono::DateTime::parse_from_rfc3339(&occurred)
            .map(|t| (now.timestamp() - t.timestamp()).max(0) as f64 / 86_400.0)
            .unwrap_or(f64::MAX);
        if let Some(s) = rrf.get_mut(&id) {
            *s *= 1.0 + 0.1 * (-age_days / 30.0).exp();
        }
    }

    let mut scored: Vec<(i64, f64)> = rrf.into_iter().collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k as usize);

    // 取全文并按预算裁剪（至少返回第一条命中）
    let mut out = Vec::new();
    let mut used = 0usize;
    for (id, _) in scored {
        let Some(entry) = repo::get_entry(ctx, id).await? else {
            continue;
        };
        let cost = entry.title.chars().count() + entry.body.chars().count();
        if !out.is_empty() && used + cost > max_chars {
            break;
        }
        used += cost;
        out.push(entry);
    }
    Ok(out)
}
