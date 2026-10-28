use anyhow::Context as _;
use sqlx::SqlitePool;

use crate::ctx::Ctx;
use crate::embed;
use crate::model::{now, AuditRow, Entity, Entry, FactRow, FactUpsert, NewEntry};

pub(crate) async fn audit(
    pool: &SqlitePool,
    action: &str,
    object_type: &str,
    object_id: Option<i64>,
    source: &str,
    summary: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO audit_log (at, action, object_type, object_id, source, summary) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(now())
    .bind(action)
    .bind(object_type)
    .bind(object_id)
    .bind(source)
    .bind(summary)
    .execute(pool)
    .await?;
    Ok(())
}

/// 种子：代表用户本人的实体（幂等）。
/// 先查再插而不是 INSERT OR IGNORE：IGNORE 冲突时也会消耗 AUTOINCREMENT id。
pub async fn seed_self_entity(pool: &SqlitePool) -> anyhow::Result<()> {
    if find_entity_by_name(pool, "我").await?.is_some() {
        return Ok(());
    }
    let ts = now();
    sqlx::query(
        "INSERT INTO entities (name, kind, status, created_at, updated_at) \
         VALUES ('我', 'self', 'active', ?1, ?2)",
    )
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_entity_by_name(pool: &SqlitePool, name: &str) -> anyhow::Result<Option<Entity>> {
    let entity = sqlx::query_as::<_, Entity>(
        "SELECT id, name, kind, domain, status, created_at, updated_at \
         FROM entities WHERE name = ?1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(entity)
}

pub async fn get_entity(ctx: &Ctx, id: i64) -> anyhow::Result<Entity> {
    sqlx::query_as::<_, Entity>(
        "SELECT id, name, kind, domain, status, created_at, updated_at \
         FROM entities WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await?
    .context("实体不存在")
}

pub async fn add_entity(
    ctx: &Ctx,
    name: &str,
    kind: &str,
    domain: Option<&str>,
) -> anyhow::Result<Entity> {
    if find_entity_by_name(&ctx.pool, name).await?.is_some() {
        anyhow::bail!("实体已存在：{name}");
    }
    let ts = now();
    let res = sqlx::query(
        "INSERT INTO entities (name, kind, domain, status, created_at, updated_at) \
         VALUES (?1, ?2, ?3, 'active', ?4, ?5)",
    )
    .bind(name)
    .bind(kind)
    .bind(domain)
    .bind(&ts)
    .bind(&ts)
    .execute(&ctx.pool)
    .await?;
    let id = res.last_insert_rowid();
    audit(
        &ctx.pool,
        "add_entity",
        "entity",
        Some(id),
        "cli",
        &format!("新增实体 {name}（{kind}）"),
    )
    .await?;
    get_entity(ctx, id).await
}

pub async fn list_entities(ctx: &Ctx) -> anyhow::Result<Vec<Entity>> {
    Ok(sqlx::query_as::<_, Entity>(
        "SELECT id, name, kind, domain, status, created_at, updated_at \
         FROM entities ORDER BY id",
    )
    .fetch_all(&ctx.pool)
    .await?)
}

/// 找不到就自动建 topic 实体，让「对话即采集」的链路不中断。
async fn resolve_entity(ctx: &Ctx, name: &str) -> anyhow::Result<Entity> {
    match find_entity_by_name(&ctx.pool, name).await? {
        Some(e) => Ok(e),
        None => add_entity(ctx, name, "topic", None).await,
    }
}

pub async fn get_fact(ctx: &Ctx, id: i64) -> anyhow::Result<FactRow> {
    sqlx::query_as::<_, FactRow>(
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id \
         FROM facts WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await?
    .context("事实不存在")
}

/// 版本化覆盖：旧当前值封口（valid_to），插入新值，历史永远保留。
pub async fn upsert_fact(
    ctx: &Ctx,
    input: FactUpsert,
) -> anyhow::Result<(FactRow, Option<FactRow>)> {
    let entity = resolve_entity(ctx, &input.entity_name).await?;
    let ts = now();
    let mut tx = ctx.pool.begin().await?;
    let superseded = sqlx::query_as::<_, FactRow>(
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id \
         FROM facts WHERE entity_id = ?1 AND key = ?2 AND valid_to IS NULL",
    )
    .bind(entity.id)
    .bind(&input.key)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(old) = &superseded {
        sqlx::query("UPDATE facts SET valid_to = ?1, updated_at = ?1 WHERE id = ?2")
            .bind(&ts)
            .bind(old.id)
            .execute(&mut *tx)
            .await?;
    }
    let res = sqlx::query(
        "INSERT INTO facts (entity_id, key, value, domain, sensitivity, valid_from, source_entry_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?6, ?6)",
    )
    .bind(entity.id)
    .bind(&input.key)
    .bind(input.value.to_string())
    .bind(&input.domain)
    .bind(&input.sensitivity)
    .bind(&ts)
    .bind(input.source_entry_id)
    .execute(&mut *tx)
    .await?;
    let id = res.last_insert_rowid();
    tx.commit().await?;
    audit(
        &ctx.pool,
        "upsert_fact",
        "fact",
        Some(id),
        "cli",
        &format!("{} 的 {} 已更新", entity.name, input.key),
    )
    .await?;
    Ok((get_fact(ctx, id).await?, superseded))
}

pub async fn list_facts(
    ctx: &Ctx,
    entity_id: Option<i64>,
    active_only: bool,
) -> anyhow::Result<Vec<FactRow>> {
    let sql = if active_only {
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id \
         FROM facts WHERE (?1 IS NULL OR entity_id = ?1) AND valid_to IS NULL \
         ORDER BY domain, key, id"
    } else {
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id \
         FROM facts WHERE (?1 IS NULL OR entity_id = ?1) \
         ORDER BY entity_id, key, valid_from"
    };
    Ok(sqlx::query_as::<_, FactRow>(sql)
        .bind(entity_id)
        .fetch_all(&ctx.pool)
        .await?)
}

#[derive(sqlx::FromRow)]
struct EntryRaw {
    id: i64,
    occurred_at: String,
    kind: String,
    title: String,
    body: String,
    tags: String,
    created_at: String,
}

async fn finish_entries(pool: &SqlitePool, rows: Vec<EntryRaw>) -> anyhow::Result<Vec<Entry>> {
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let entity_ids: Vec<i64> =
            sqlx::query_scalar("SELECT entity_id FROM entry_entities WHERE entry_id = ?1")
                .bind(r.id)
                .fetch_all(pool)
                .await?;
        out.push(Entry {
            id: r.id,
            occurred_at: r.occurred_at,
            kind: r.kind,
            title: r.title,
            body: r.body,
            tags: serde_json::from_str(&r.tags).unwrap_or_default(),
            entity_ids,
            created_at: r.created_at,
        });
    }
    Ok(out)
}

pub async fn get_entry(ctx: &Ctx, id: i64) -> anyhow::Result<Option<Entry>> {
    let Some(row) = sqlx::query_as::<_, EntryRaw>(
        "SELECT id, occurred_at, kind, title, body, tags, created_at FROM entries WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await?
    else {
        return Ok(None);
    };
    Ok(finish_entries(&ctx.pool, vec![row])
        .await?
        .into_iter()
        .next())
}

pub async fn add_entry(ctx: &Ctx, input: NewEntry, source: &str) -> anyhow::Result<Entry> {
    let ts = now();
    let occurred = input.occurred_at.clone().unwrap_or_else(|| ts.clone());
    let tags = serde_json::to_string(&input.tags)?;
    let res = sqlx::query(
        "INSERT INTO entries (occurred_at, kind, title, body, tags, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&occurred)
    .bind(&input.kind)
    .bind(&input.title)
    .bind(&input.body)
    .bind(&tags)
    .bind(&ts)
    .execute(&ctx.pool)
    .await?;
    let entry_id = res.last_insert_rowid();
    for name in &input.entity_names {
        let entity = resolve_entity(ctx, name).await?;
        sqlx::query("INSERT OR IGNORE INTO entry_entities (entry_id, entity_id) VALUES (?1, ?2)")
            .bind(entry_id)
            .bind(entity.id)
            .execute(&ctx.pool)
            .await?;
    }
    let entity_ids: Vec<i64> =
        sqlx::query_scalar("SELECT entity_id FROM entry_entities WHERE entry_id = ?1")
            .bind(entry_id)
            .fetch_all(&ctx.pool)
            .await?;
    audit(
        &ctx.pool,
        "add_entry",
        "entry",
        Some(entry_id),
        source,
        &input.title,
    )
    .await?;
    embed::store_entry_embedding(ctx, entry_id, &input.title, &input.body).await;
    Ok(Entry {
        id: entry_id,
        occurred_at: occurred,
        kind: input.kind,
        title: input.title,
        body: input.body,
        tags: input.tags,
        entity_ids,
        created_at: ts,
    })
}

pub async fn recent_entries(ctx: &Ctx, limit: i64) -> anyhow::Result<Vec<Entry>> {
    let rows = sqlx::query_as::<_, EntryRaw>(
        "SELECT id, occurred_at, kind, title, body, tags, created_at FROM entries \
         ORDER BY occurred_at DESC, id DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(&ctx.pool)
    .await?;
    finish_entries(&ctx.pool, rows).await
}

pub async fn list_audit(ctx: &Ctx, limit: i64) -> anyhow::Result<Vec<AuditRow>> {
    Ok(sqlx::query_as::<_, AuditRow>(
        "SELECT id, at, action, object_type, object_id, source, summary \
         FROM audit_log ORDER BY id DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(&ctx.pool)
    .await?)
}

/// 概览计数（桌面端 db_info 用）。
pub async fn counts(ctx: &Ctx) -> anyhow::Result<(i64, i64, i64)> {
    let entities: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entities")
        .fetch_one(&ctx.pool)
        .await?;
    let facts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM facts WHERE valid_to IS NULL")
        .fetch_one(&ctx.pool)
        .await?;
    let entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
        .fetch_one(&ctx.pool)
        .await?;
    Ok((entities, facts, entries))
}
