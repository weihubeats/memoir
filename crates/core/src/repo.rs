use anyhow::Context as _;
use serde::Serialize;
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
        "SELECT id, name, kind, domain, status, created_at, updated_at, deleted_at \
         FROM entities WHERE name = ?1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(entity)
}

pub async fn get_entity(ctx: &Ctx, id: i64) -> anyhow::Result<Entity> {
    sqlx::query_as::<_, Entity>(
        "SELECT id, name, kind, domain, status, created_at, updated_at, deleted_at \
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
    source: &str,
) -> anyhow::Result<Entity> {
    if let Some(existing) = find_entity_by_name(&ctx.pool, name).await? {
        if existing.deleted_at.is_some() || existing.status == "deleted" {
            anyhow::bail!("实体在回收站中：{name}（先恢复再操作）");
        }
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
        source,
        &format!("新增实体 {name}（{kind}）"),
    )
    .await?;
    get_entity(ctx, id).await
}

pub async fn list_entities(ctx: &Ctx) -> anyhow::Result<Vec<Entity>> {
    Ok(sqlx::query_as::<_, Entity>(
        "SELECT id, name, kind, domain, status, created_at, updated_at, deleted_at \
         FROM entities WHERE status != 'deleted' AND deleted_at IS NULL ORDER BY id",
    )
    .fetch_all(&ctx.pool)
    .await?)
}

/// 找不到就自动建 topic 实体；名字在回收站里则先恢复，让「对话即采集」不中断。
async fn resolve_entity(ctx: &Ctx, name: &str) -> anyhow::Result<Entity> {
    match find_entity_by_name(&ctx.pool, name).await? {
        Some(e) if e.deleted_at.is_some() || e.status == "deleted" => {
            restore_entity(ctx, e.id, "auto").await?;
            get_entity(ctx, e.id).await
        }
        Some(e) => Ok(e),
        None => add_entity(ctx, name, "topic", None, "auto").await,
    }
}

pub async fn get_fact(ctx: &Ctx, id: i64) -> anyhow::Result<FactRow> {
    sqlx::query_as::<_, FactRow>(
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id, deleted_at \
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
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id, deleted_at \
         FROM facts \
         WHERE entity_id = ?1 AND key = ?2 AND valid_to IS NULL AND deleted_at IS NULL",
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
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id, deleted_at \
         FROM facts \
         WHERE (?1 IS NULL OR entity_id = ?1) AND valid_to IS NULL AND deleted_at IS NULL \
         ORDER BY domain, key, id"
    } else {
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id, deleted_at \
         FROM facts \
         WHERE (?1 IS NULL OR entity_id = ?1) AND deleted_at IS NULL \
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
    deleted_at: Option<String>,
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
            deleted_at: r.deleted_at,
        });
    }
    Ok(out)
}

pub async fn get_entry(ctx: &Ctx, id: i64) -> anyhow::Result<Option<Entry>> {
    let Some(row) = sqlx::query_as::<_, EntryRaw>(
        "SELECT id, occurred_at, kind, title, body, tags, created_at, deleted_at \
         FROM entries WHERE id = ?1",
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
        deleted_at: None,
    })
}

pub async fn recent_entries(ctx: &Ctx, limit: i64) -> anyhow::Result<Vec<Entry>> {
    let rows = sqlx::query_as::<_, EntryRaw>(
        "SELECT id, occurred_at, kind, title, body, tags, created_at, deleted_at \
         FROM entries WHERE deleted_at IS NULL \
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
    let entities: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entities WHERE status != 'deleted' AND deleted_at IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await?;
    let facts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM facts WHERE valid_to IS NULL AND deleted_at IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await?;
    let entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE deleted_at IS NULL")
        .fetch_one(&ctx.pool)
        .await?;
    Ok((entities, facts, entries))
}

// ---------- 回收站（设计文档 §6.3：软删 30 天可恢复，彻底清除物理删除） ----------

/// 回收站内容。
#[derive(Debug, Serialize)]
pub struct TrashData {
    pub entities: Vec<Entity>,
    pub facts: Vec<FactRow>,
    pub entries: Vec<Entry>,
}

pub async fn list_trash(ctx: &Ctx) -> anyhow::Result<TrashData> {
    let entities = sqlx::query_as::<_, Entity>(
        "SELECT id, name, kind, domain, status, created_at, updated_at, deleted_at \
         FROM entities \
         WHERE status = 'deleted' OR deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(&ctx.pool)
    .await?;
    let fact_rows = sqlx::query_as::<_, FactRow>(
        "SELECT id, entity_id, key, value, domain, sensitivity, valid_from, valid_to, source_entry_id, deleted_at \
         FROM facts WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(&ctx.pool)
    .await?;
    let entry_rows = sqlx::query_as::<_, EntryRaw>(
        "SELECT id, occurred_at, kind, title, body, tags, created_at, deleted_at \
         FROM entries WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(&ctx.pool)
    .await?;
    Ok(TrashData {
        entities,
        facts: fact_rows,
        entries: finish_entries(&ctx.pool, entry_rows).await?,
    })
}

pub async fn delete_entry(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    let entry = get_entry(ctx, id).await?.context("记录不存在")?;
    sqlx::query("UPDATE entries SET deleted_at = ?1 WHERE id = ?2")
        .bind(now())
        .bind(id)
        .execute(&ctx.pool)
        .await?;
    audit(
        &ctx.pool,
        "delete_entry",
        "entry",
        Some(id),
        source,
        &entry.title,
    )
    .await
}

pub async fn restore_entry(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE entries SET deleted_at = NULL WHERE id = ?1")
        .bind(id)
        .execute(&ctx.pool)
        .await?;
    audit(&ctx.pool, "restore_entry", "entry", Some(id), source, "").await
}

pub async fn purge_entry(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    let title = get_entry(ctx, id)
        .await?
        .map(|e| e.title)
        .unwrap_or_default();
    sqlx::query("DELETE FROM entries WHERE id = ?1")
        .bind(id)
        .execute(&ctx.pool)
        .await?;
    audit(&ctx.pool, "purge_entry", "entry", Some(id), source, &title).await
}

/// 软删事实：封口 + 进回收站。
pub async fn delete_fact(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    let fact = get_fact(ctx, id).await?;
    if fact.valid_to.is_some() || fact.deleted_at.is_some() {
        anyhow::bail!("仅当前有效的事实可删除");
    }
    let ts = now();
    sqlx::query("UPDATE facts SET valid_to = ?1, deleted_at = ?1, updated_at = ?1 WHERE id = ?2")
        .bind(&ts)
        .bind(id)
        .execute(&ctx.pool)
        .await?;
    audit(
        &ctx.pool,
        "delete_fact",
        "fact",
        Some(id),
        source,
        &fact.key,
    )
    .await
}

/// 恢复事实：同实体同 key 已有其他有效事实时拒绝（避免双活）。
pub async fn restore_fact(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    let fact = get_fact(ctx, id).await?;
    if fact.deleted_at.is_none() {
        anyhow::bail!("事实不在回收站中");
    }
    let conflicting: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM facts \
         WHERE entity_id = ?1 AND key = ?2 AND valid_to IS NULL AND deleted_at IS NULL AND id != ?3",
    )
    .bind(fact.entity_id)
    .bind(&fact.key)
    .bind(id)
    .fetch_one(&ctx.pool)
    .await?;
    if conflicting > 0 {
        anyhow::bail!("同实体同键已有生效事实，无法恢复（先处理现有事实）");
    }
    sqlx::query(
        "UPDATE facts SET deleted_at = NULL, valid_to = NULL, updated_at = ?1 WHERE id = ?2",
    )
    .bind(now())
    .bind(id)
    .execute(&ctx.pool)
    .await?;
    audit(
        &ctx.pool,
        "restore_fact",
        "fact",
        Some(id),
        source,
        &fact.key,
    )
    .await
}

pub async fn purge_fact(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    let fact = get_fact(ctx, id).await?;
    sqlx::query("DELETE FROM facts WHERE id = ?1")
        .bind(id)
        .execute(&ctx.pool)
        .await?;
    audit(&ctx.pool, "purge_fact", "fact", Some(id), source, &fact.key).await
}

pub async fn delete_entity(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    let entity = get_entity(ctx, id).await?;
    let ts = now();
    sqlx::query(
        "UPDATE entities SET status = 'deleted', deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
    )
    .bind(&ts)
    .bind(id)
    .execute(&ctx.pool)
    .await?;
    audit(
        &ctx.pool,
        "delete_entity",
        "entity",
        Some(id),
        source,
        &entity.name,
    )
    .await
}

pub async fn restore_entity(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    let entity = get_entity(ctx, id).await?;
    sqlx::query(
        "UPDATE entities SET status = 'active', deleted_at = NULL, updated_at = ?1 WHERE id = ?2",
    )
    .bind(now())
    .bind(id)
    .execute(&ctx.pool)
    .await?;
    audit(
        &ctx.pool,
        "restore_entity",
        "entity",
        Some(id),
        source,
        &entity.name,
    )
    .await
}

/// 彻底清除实体：级联物理删除其全部事实与关联（记录本身保留）。
pub async fn purge_entity(ctx: &Ctx, id: i64, source: &str) -> anyhow::Result<()> {
    let entity = get_entity(ctx, id).await?;
    sqlx::query("DELETE FROM entities WHERE id = ?1")
        .bind(id)
        .execute(&ctx.pool)
        .await?;
    audit(
        &ctx.pool,
        "purge_entity",
        "entity",
        Some(id),
        source,
        &format!("{}（级联删除事实与关联）", entity.name),
    )
    .await
}

/// 合并实体：source 的事实与关联全部移到 target，冲突键保留最新、旧值封口，source 物理删除。
pub async fn merge_entities(
    ctx: &Ctx,
    source_id: i64,
    target_id: i64,
    source: &str,
) -> anyhow::Result<Entity> {
    if source_id == target_id {
        anyhow::bail!("不能合并到自身");
    }
    let from = get_entity(ctx, source_id).await?;
    let to = get_entity(ctx, target_id).await?;

    {
        let mut tx = ctx.pool.begin().await?;
        sqlx::query("UPDATE facts SET entity_id = ?1 WHERE entity_id = ?2")
            .bind(target_id)
            .bind(source_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT OR IGNORE INTO entry_entities (entry_id, entity_id) \
             SELECT entry_id, ?1 FROM entry_entities WHERE entity_id = ?2",
        )
        .bind(target_id)
        .bind(source_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM entry_entities WHERE entity_id = ?1")
            .bind(source_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }

    // 冲突键：同键多条同时有效 → 保留最新，其余封口
    let conflicts: Vec<String> = sqlx::query_scalar(
        "SELECT key FROM facts \
         WHERE entity_id = ?1 AND valid_to IS NULL AND deleted_at IS NULL \
         GROUP BY key HAVING COUNT(*) > 1",
    )
    .bind(target_id)
    .fetch_all(&ctx.pool)
    .await?;
    for key in conflicts {
        let keep: i64 = sqlx::query_scalar(
            "SELECT id FROM facts \
             WHERE entity_id = ?1 AND key = ?2 AND valid_to IS NULL AND deleted_at IS NULL \
             ORDER BY valid_from DESC, id DESC LIMIT 1",
        )
        .bind(target_id)
        .bind(&key)
        .fetch_one(&ctx.pool)
        .await?;
        sqlx::query(
            "UPDATE facts SET valid_to = ?1, updated_at = ?1 \
             WHERE entity_id = ?2 AND key = ?3 AND valid_to IS NULL AND deleted_at IS NULL AND id != ?4",
        )
        .bind(now())
        .bind(target_id)
        .bind(&key)
        .bind(keep)
        .execute(&ctx.pool)
        .await?;
    }

    sqlx::query("DELETE FROM entities WHERE id = ?1")
        .bind(source_id)
        .execute(&ctx.pool)
        .await?;
    audit(
        &ctx.pool,
        "merge_entities",
        "entity",
        Some(target_id),
        source,
        &format!("{} 并入 {}", from.name, to.name),
    )
    .await?;
    get_entity(ctx, target_id).await
}

/// 回收站 30 天自动物理清理（打开库时执行，幂等）。
pub async fn auto_purge(pool: &SqlitePool) -> anyhow::Result<(u64, u64)> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(30))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let entries =
        sqlx::query("DELETE FROM entries WHERE deleted_at IS NOT NULL AND deleted_at < ?1")
            .bind(&cutoff)
            .execute(pool)
            .await?
            .rows_affected();
    let facts = sqlx::query("DELETE FROM facts WHERE deleted_at IS NOT NULL AND deleted_at < ?1")
        .bind(&cutoff)
        .execute(pool)
        .await?
        .rows_affected();
    Ok((entries, facts))
}

// ---------- 内置对话（M4） ----------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub backend: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ChatMessageRow {
    pub id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

pub async fn create_conversation(ctx: &Ctx, backend: &str) -> anyhow::Result<Conversation> {
    let ts = now();
    let res = sqlx::query(
        "INSERT INTO conversations (title, backend, created_at, updated_at) \
         VALUES ('新对话', ?1, ?2, ?3)",
    )
    .bind(backend)
    .bind(&ts)
    .bind(&ts)
    .execute(&ctx.pool)
    .await?;
    sqlx::query_as::<_, Conversation>(
        "SELECT id, title, backend, created_at, updated_at \
         FROM conversations WHERE id = ?1",
    )
    .bind(res.last_insert_rowid())
    .fetch_one(&ctx.pool)
    .await
    .context("会话创建后读取失败")
}

pub async fn list_conversations(ctx: &Ctx) -> anyhow::Result<Vec<Conversation>> {
    Ok(sqlx::query_as::<_, Conversation>(
        "SELECT id, title, backend, created_at, updated_at \
         FROM conversations ORDER BY updated_at DESC",
    )
    .fetch_all(&ctx.pool)
    .await?)
}

pub async fn get_conversation(ctx: &Ctx, id: i64) -> anyhow::Result<Conversation> {
    sqlx::query_as::<_, Conversation>(
        "SELECT id, title, backend, created_at, updated_at \
         FROM conversations WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&ctx.pool)
    .await?
    .context("会话不存在")
}

pub async fn delete_conversation(ctx: &Ctx, id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM conversations WHERE id = ?1")
        .bind(id)
        .execute(&ctx.pool)
        .await?;
    audit(
        &ctx.pool,
        "delete_conversation",
        "conversation",
        Some(id),
        "app",
        "",
    )
    .await
}

/// 追加消息；首条用户消息自动作为会话标题（截前 24 字）。
pub async fn add_chat_message(
    ctx: &Ctx,
    conversation_id: i64,
    role: &str,
    content: &str,
) -> anyhow::Result<i64> {
    let ts = now();
    let res = sqlx::query(
        "INSERT INTO messages (conversation_id, role, content, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(conversation_id)
    .bind(role)
    .bind(content)
    .bind(&ts)
    .execute(&ctx.pool)
    .await?;
    if role == "user" {
        sqlx::query(
            "UPDATE conversations SET updated_at = ?1, \
             title = CASE WHEN title = '新对话' THEN substr(?2, 1, 24) ELSE title END \
             WHERE id = ?3",
        )
        .bind(&ts)
        .bind(content)
        .bind(conversation_id)
        .execute(&ctx.pool)
        .await?;
    } else {
        sqlx::query("UPDATE conversations SET updated_at = ?1 WHERE id = ?2")
            .bind(&ts)
            .bind(conversation_id)
            .execute(&ctx.pool)
            .await?;
    }
    Ok(res.last_insert_rowid())
}

pub async fn chat_messages(ctx: &Ctx, conversation_id: i64) -> anyhow::Result<Vec<ChatMessageRow>> {
    Ok(sqlx::query_as::<_, ChatMessageRow>(
        "SELECT id, conversation_id, role, content, created_at \
         FROM messages WHERE conversation_id = ?1 ORDER BY id",
    )
    .bind(conversation_id)
    .fetch_all(&ctx.pool)
    .await?)
}

/// 记录「用户已采纳该写操作」。
pub async fn adopt_chat_op(ctx: &Ctx, message_id: i64, op_index: i64) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO chat_op_adoptions (message_id, op_index, adopted_at) \
         VALUES (?1, ?2, ?3)",
    )
    .bind(message_id)
    .bind(op_index)
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

pub async fn adopted_chat_ops(ctx: &Ctx, message_id: i64) -> anyhow::Result<Vec<i64>> {
    sqlx::query_scalar::<_, i64>("SELECT op_index FROM chat_op_adoptions WHERE message_id = ?1")
        .bind(message_id)
        .fetch_all(&ctx.pool)
        .await
        .map_err(Into::into)
}

// ---------- 应用设置（KV） ----------

pub async fn settings_get(pool: &SqlitePool, key: &str) -> anyhow::Result<Option<String>> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}

pub async fn settings_set(pool: &SqlitePool, key: &str, value: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = ?2",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn settings_all(pool: &SqlitePool) -> anyhow::Result<Vec<(String, String)>> {
    sqlx::query_as::<_, (String, String)>("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}
