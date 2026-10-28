use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::model::FactRow;
use crate::{profile, repo};

/// 全库 JSON 导出（数据主权兜底），含历史与审计。
pub async fn export_json(pool: &SqlitePool) -> anyhow::Result<Value> {
    let entities = repo::list_entities(pool).await?;
    let facts = repo::list_facts(pool, None, false).await?;
    let entries = repo::recent_entries(pool, 1_000_000).await?;
    let audit = repo::list_audit(pool, 1_000_000).await?;
    let facts: Vec<Value> = facts
        .iter()
        .map(|f: &FactRow| {
            json!({
                "id": f.id,
                "entity_id": f.entity_id,
                "key": f.key,
                "value": f.value_json(),
                "domain": f.domain,
                "sensitivity": f.sensitivity,
                "valid_from": f.valid_from,
                "valid_to": f.valid_to,
                "source_entry_id": f.source_entry_id,
            })
        })
        .collect();
    Ok(json!({
        "entities": entities,
        "facts": facts,
        "entries": entries,
        "audit_log": audit,
    }))
}

pub async fn export_markdown(pool: &SqlitePool) -> anyhow::Result<String> {
    let mut out = String::from("# my-context 全库导出\n\n");
    out.push_str("## 档案（当前有效事实）\n\n");
    out.push_str(
        &profile::get_profile(
            pool,
            profile::ProfileOptions {
                include_sensitive: true,
                include_secret: true,
                ..Default::default()
            },
        )
        .await?,
    );
    out.push_str("\n\n## 记录\n\n");
    for e in repo::recent_entries(pool, 1_000_000).await? {
        out.push_str(&format!(
            "### {} — {}（{}）\n\n{}\n\n",
            e.occurred_at, e.title, e.kind, e.body
        ));
    }
    Ok(out)
}
