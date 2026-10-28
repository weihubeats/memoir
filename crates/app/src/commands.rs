use my_context_core as core;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;

fn err(e: anyhow::Error) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct DbInfo {
    pub path: String,
    pub entities: i64,
    pub facts: i64,
    pub entries: i64,
}

#[tauri::command]
pub async fn db_info(state: State<'_, AppState>) -> Result<DbInfo, String> {
    let (entities, facts, entries) = core::repo::counts(&state.ctx).await.map_err(err)?;
    Ok(DbInfo {
        path: state.db_path.clone(),
        entities,
        facts,
        entries,
    })
}

#[tauri::command]
pub async fn list_entities(state: State<'_, AppState>) -> Result<Vec<core::model::Entity>, String> {
    core::repo::list_entities(&state.ctx).await.map_err(err)
}

#[tauri::command]
pub async fn list_facts(
    state: State<'_, AppState>,
    active_only: Option<bool>,
) -> Result<Vec<core::model::FactRow>, String> {
    core::repo::list_facts(&state.ctx, None, active_only.unwrap_or(true))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn list_entries(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<core::model::Entry>, String> {
    core::repo::recent_entries(&state.ctx, limit.unwrap_or(50))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn list_audit(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<core::model::AuditRow>, String> {
    core::repo::list_audit(&state.ctx, limit.unwrap_or(50))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn stale_facts(
    state: State<'_, AppState>,
) -> Result<Vec<core::report::StaleFact>, String> {
    core::report::list_stale_facts(&state.ctx.pool, core::report::STALE_MONTHS)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn search_entries(
    state: State<'_, AppState>,
    query: String,
    top_k: Option<i64>,
) -> Result<Vec<core::model::Entry>, String> {
    core::search::search_entries(
        &state.ctx,
        &query,
        top_k.unwrap_or(core::search::DEFAULT_TOP_K),
        core::search::DEFAULT_MAX_CHARS,
    )
    .await
    .map_err(err)
}

#[derive(Deserialize)]
pub struct UpsertFactArgs {
    pub entity: String,
    pub key: String,
    pub value: String,
    pub domain: Option<String>,
    pub sensitivity: Option<String>,
}

#[tauri::command]
pub async fn upsert_fact(
    state: State<'_, AppState>,
    args: UpsertFactArgs,
) -> Result<core::model::FactRow, String> {
    let (fact, _) = core::repo::upsert_fact(
        &state.ctx,
        core::model::FactUpsert {
            entity_name: args.entity,
            key: args.key,
            value: core::model::coerce_value(&args.value),
            domain: args.domain,
            sensitivity: args.sensitivity.unwrap_or_else(|| "normal".to_owned()),
            source_entry_id: None,
        },
    )
    .await
    .map_err(err)?;
    Ok(fact)
}

#[derive(Deserialize)]
pub struct AddEntryArgs {
    pub title: String,
    pub body: String,
    pub kind: Option<String>,
    pub tags: Option<Vec<String>>,
    pub entities: Option<Vec<String>>,
}

#[tauri::command]
pub async fn add_entry(
    state: State<'_, AppState>,
    args: AddEntryArgs,
) -> Result<core::model::Entry, String> {
    core::repo::add_entry(
        &state.ctx,
        core::model::NewEntry {
            kind: args.kind.unwrap_or_else(|| "note".to_owned()),
            title: args.title,
            body: args.body,
            tags: args.tags.unwrap_or_default(),
            entity_names: args.entities.unwrap_or_default(),
            occurred_at: None,
        },
        "app",
    )
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn add_entity(
    state: State<'_, AppState>,
    name: String,
    kind: Option<String>,
    domain: Option<String>,
) -> Result<core::model::Entity, String> {
    core::repo::add_entity(
        &state.ctx,
        &name,
        kind.as_deref().unwrap_or("topic"),
        domain.as_deref(),
    )
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn monthly_report(
    state: State<'_, AppState>,
    year: Option<i32>,
    month: Option<u32>,
) -> Result<String, String> {
    let (cy, cm) = core::report::current_month();
    core::report::monthly_report(&state.ctx, year.unwrap_or(cy), month.unwrap_or(cm))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn export_data(state: State<'_, AppState>, format: String) -> Result<String, String> {
    match format.as_str() {
        "json" => {
            serde_json::to_string_pretty(&core::export::export_json(&state.ctx).await.map_err(err)?)
                .map_err(|e| e.to_string())
        }
        "markdown" | "md" => core::export::export_markdown(&state.ctx).await.map_err(err),
        other => Err(format!("不支持的格式：{other}（可选 json / markdown）")),
    }
}

#[tauri::command]
pub async fn get_context_preview(
    state: State<'_, AppState>,
    query: Option<String>,
) -> Result<String, String> {
    core::context::get_context(
        &state.ctx,
        query.as_deref(),
        core::context::DEFAULT_BUDGET_CHARS,
    )
    .await
    .map_err(err)
}
