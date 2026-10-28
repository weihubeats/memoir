use chrono::Datelike;
use sqlx::SqlitePool;

use crate::Ctx;

pub const STALE_MONTHS: i64 = 6;

pub fn current_month() -> (i32, u32) {
    let now = chrono::Utc::now();
    (now.year(), now.month())
}

/// 保鲜：超过 `months` 个月未更新的当前有效事实（设计文档 §6.4）。
pub async fn list_stale_facts(
    pool: &SqlitePool,
    months: i64,
) -> anyhow::Result<Vec<(String, String, String, String)>> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(30 * months))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT e.name, f.key, f.value, f.updated_at \
         FROM facts f JOIN entities e ON e.id = f.entity_id \
         WHERE f.valid_to IS NULL AND f.updated_at < ?1 ORDER BY f.updated_at",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 月报材料（设计文档 §6.4）：本月记录 + 待确认事实 + 操作统计。
/// 叙事性总结由 AI 在对话中基于这份材料生成。
pub async fn monthly_report(ctx: &Ctx, year: i32, month: u32) -> anyhow::Result<String> {
    let start = format!("{year:04}-{month:02}-01T00:00:00Z");
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = format!("{ny:04}-{nm:02}-01T00:00:00Z");

    let entries: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, occurred_at, kind, title FROM entries \
         WHERE occurred_at >= ?1 AND occurred_at < ?2 ORDER BY occurred_at",
    )
    .bind(&start)
    .bind(&end)
    .fetch_all(&ctx.pool)
    .await?;

    let stale = list_stale_facts(&ctx.pool, STALE_MONTHS).await?;
    let audit: Vec<(String, i64)> = sqlx::query_as(
        "SELECT action, COUNT(*) FROM audit_log \
         WHERE at >= ?1 AND at < ?2 GROUP BY action ORDER BY 2 DESC",
    )
    .bind(&start)
    .bind(&end)
    .fetch_all(&ctx.pool)
    .await?;

    let mut out = format!("# {year:04}-{month:02} 上下文月报（材料）\n\n");
    out.push_str(&format!("## 本月记录（{} 条）\n\n", entries.len()));
    if entries.is_empty() {
        out.push_str("（无）\n");
    }
    for (id, at, kind, title) in &entries {
        out.push_str(&format!("- #{id} [{kind}] {at} {title}\n"));
    }
    out.push_str(&format!(
        "\n## 待确认事实（超过 {STALE_MONTHS} 个月未更新）\n\n"
    ));
    if stale.is_empty() {
        out.push_str("（无）\n");
    }
    for (entity, key, value, at) in &stale {
        out.push_str(&format!("- {entity} | {key} = {value}（最后更新 {at}）\n"));
    }
    out.push_str("\n## 本月操作统计\n\n");
    if audit.is_empty() {
        out.push_str("（无）\n");
    }
    for (action, n) in &audit {
        out.push_str(&format!("- {action}: {n}\n"));
    }
    Ok(out)
}
