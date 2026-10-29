use memoir_core as core;
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
        "app",
    )
    .await
    .map_err(err)
}

// ---------- 回收站（软删 / 恢复 / 彻底清除）----------

#[tauri::command]
pub async fn list_trash(state: State<'_, AppState>) -> Result<core::repo::TrashData, String> {
    core::repo::list_trash(&state.ctx).await.map_err(err)
}

#[tauri::command]
pub async fn delete_entry(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::delete_entry(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn restore_entry(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::restore_entry(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn purge_entry(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::purge_entry(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn delete_fact(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::delete_fact(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn restore_fact(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::restore_fact(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn purge_fact(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::purge_fact(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn delete_entity(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::delete_entity(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn restore_entity(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::restore_entity(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn purge_entity(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::purge_entity(&state.ctx, id, "app")
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn merge_entities(
    state: State<'_, AppState>,
    source_id: i64,
    target_id: i64,
) -> Result<core::model::Entity, String> {
    core::repo::merge_entities(&state.ctx, source_id, target_id, "app")
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

// ---------- 内置对话（M4） ----------

use core::chat::ChatOp;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
struct ChatStartPayload {
    conversation_id: i64,
    user_message_id: i64,
}

#[derive(Serialize, Clone)]
struct ChatChunkPayload {
    conversation_id: i64,
    delta: String,
}

#[derive(Serialize, Clone)]
struct ChatDonePayload {
    conversation_id: i64,
    message_id: i64,
}

#[derive(Serialize, Clone)]
struct ChatErrorPayload {
    conversation_id: i64,
    error: String,
}

#[derive(Serialize)]
pub struct ChatMessageDto {
    pub id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub segments: Vec<core::chat::Segment>,
    pub adopted: Vec<i64>,
}

async fn get_setting(ctx: &core::Ctx, key: &str) -> Result<String, String> {
    Ok(core::repo::settings_get(&ctx.pool, key)
        .await
        .map_err(err)?
        .unwrap_or_default())
}

async fn resolve_llm_config(
    ctx: &core::Ctx,
    backend: &str,
) -> Result<core::chat::LlmConfig, String> {
    match backend {
        "local" => Ok(core::chat::LlmConfig {
            base_url: match get_setting(ctx, "local_base_url").await? {
                u if u.is_empty() => "http://127.0.0.1:11434/v1".to_owned(),
                u => u,
            },
            api_key: get_setting(ctx, "local_api_key").await?,
            model: get_setting(ctx, "local_model").await?,
        }),
        _ => {
            let base_url = get_setting(ctx, "cloud_base_url").await?;
            let model = get_setting(ctx, "cloud_model").await?;
            if base_url.is_empty() || model.is_empty() {
                return Err(
                    "云端模型未配置：请到「设置 → 对话后端」填写 Base URL 与模型名".to_owned(),
                );
            }
            Ok(core::chat::LlmConfig {
                base_url,
                api_key: get_setting(ctx, "cloud_api_key").await?,
                model,
            })
        }
    }
}

#[tauri::command]
pub async fn chat_new(
    state: State<'_, AppState>,
    backend: Option<String>,
) -> Result<core::repo::Conversation, String> {
    core::repo::create_conversation(&state.ctx, backend.as_deref().unwrap_or("cloud"))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn chat_list(
    state: State<'_, AppState>,
) -> Result<Vec<core::repo::Conversation>, String> {
    core::repo::list_conversations(&state.ctx)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn chat_delete(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    core::repo::delete_conversation(&state.ctx, id)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn chat_messages(
    state: State<'_, AppState>,
    conversation_id: i64,
) -> Result<Vec<ChatMessageDto>, String> {
    let rows = core::repo::chat_messages(&state.ctx, conversation_id)
        .await
        .map_err(err)?;
    let mut out = Vec::with_capacity(rows.len());
    for m in rows {
        let adopted = core::repo::adopted_chat_ops(&state.ctx, m.id)
            .await
            .map_err(err)?;
        let segments = core::chat::parse_segments(&m.content);
        out.push(ChatMessageDto {
            id: m.id,
            conversation_id: m.conversation_id,
            role: m.role,
            content: m.content,
            created_at: m.created_at,
            segments,
            adopted,
        });
    }
    Ok(out)
}

#[derive(serde::Deserialize)]
pub struct ChatSendArgs {
    pub conversation_id: i64,
    pub content: String,
}

/// 发送消息：立即落库用户消息并返回，LLM 流式回复通过事件推送
/// （memoir://chat-start / chat-chunk / chat-done / chat-error）。
#[tauri::command]
pub async fn chat_send(
    state: State<'_, AppState>,
    app: AppHandle,
    args: ChatSendArgs,
) -> Result<i64, String> {
    let conversation = core::repo::get_conversation(&state.ctx, args.conversation_id)
        .await
        .map_err(err)?;
    let user_message_id =
        core::repo::add_chat_message(&state.ctx, args.conversation_id, "user", &args.content)
            .await
            .map_err(err)?;
    let _ = app.emit(
        "memoir://chat-start",
        ChatStartPayload {
            conversation_id: args.conversation_id,
            user_message_id,
        },
    );

    // 上下文注入：档案常驻 + 按话题检索（secret 级永不注入）
    let context = core::context::get_context(
        &state.ctx,
        Some(&args.content),
        core::context::DEFAULT_BUDGET_CHARS,
    )
    .await
    .map_err(err)?;
    let history = core::repo::chat_messages(&state.ctx, args.conversation_id)
        .await
        .map_err(err)?;
    let mut llm_messages = vec![core::chat::LlmMessage {
        role: "system".to_owned(),
        content: format!(
            "{}\n\n---\n以下是从用户个人上下文库注入的资料：\n\n{context}",
            core::chat::SYSTEM_PROMPT
        ),
    }];
    for m in history {
        if m.role != "system" {
            llm_messages.push(core::chat::LlmMessage {
                role: m.role,
                content: m.content,
            });
        }
    }
    let cfg = resolve_llm_config(&state.ctx, &conversation.backend).await?;

    let ctx = state.ctx.clone();
    let emitter = app.clone();
    let conversation_id = args.conversation_id;
    tauri::async_runtime::spawn(async move {
        let result = core::chat::chat_stream(&cfg, &llm_messages, |delta| {
            let _ = emitter.emit(
                "memoir://chat-chunk",
                ChatChunkPayload {
                    conversation_id,
                    delta: delta.to_owned(),
                },
            );
        })
        .await;
        match result {
            Ok(text) => {
                match core::repo::add_chat_message(&ctx, conversation_id, "assistant", &text).await
                {
                    Ok(message_id) => {
                        let _ = emitter.emit(
                            "memoir://chat-done",
                            ChatDonePayload {
                                conversation_id,
                                message_id,
                            },
                        );
                    }
                    Err(e) => {
                        let _ = emitter.emit(
                            "memoir://chat-error",
                            ChatErrorPayload {
                                conversation_id,
                                error: e.to_string(),
                            },
                        );
                    }
                }
            }
            Err(e) => {
                let _ = emitter.emit(
                    "memoir://chat-error",
                    ChatErrorPayload {
                        conversation_id,
                        error: e.to_string(),
                    },
                );
            }
        }
    });
    Ok(user_message_id)
}

#[derive(serde::Deserialize)]
pub struct ApplyOpArgs {
    pub message_id: i64,
    pub op_index: i64,
    pub op: ChatOp,
}

/// 采纳 AI 写操作：真正落库并记录采纳（撤销由前端丢弃即可，不落库）。
#[tauri::command]
pub async fn apply_chat_op(
    state: State<'_, AppState>,
    args: ApplyOpArgs,
) -> Result<String, String> {
    let summary = match &args.op {
        ChatOp::UpsertFact {
            entity,
            key,
            value,
            domain,
            sensitivity,
        } => {
            let (fact, _) = core::repo::upsert_fact(
                &state.ctx,
                core::model::FactUpsert {
                    entity_name: entity.clone(),
                    key: key.clone(),
                    value: value.clone(),
                    domain: domain.clone(),
                    sensitivity: sensitivity.clone().unwrap_or_else(|| "normal".to_owned()),
                    source_entry_id: None,
                },
            )
            .await
            .map_err(err)?;
            format!("事实 #{}：{} = {}", fact.id, fact.key, fact.value)
        }
        ChatOp::AddEntry {
            title,
            body,
            kind,
            tags,
            entities,
        } => {
            let entry = core::repo::add_entry(
                &state.ctx,
                core::model::NewEntry {
                    kind: kind.clone().unwrap_or_else(|| "chat".to_owned()),
                    title: title.clone(),
                    body: body.clone(),
                    tags: tags.clone(),
                    entity_names: entities.clone(),
                    occurred_at: None,
                },
                "chat",
            )
            .await
            .map_err(err)?;
            format!("记录 #{}：{}", entry.id, entry.title)
        }
    };
    core::repo::adopt_chat_op(&state.ctx, args.message_id, args.op_index)
        .await
        .map_err(err)?;
    Ok(summary)
}

// ---------- 应用设置 ----------

#[tauri::command]
pub async fn settings_all(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(core::repo::settings_all(&state.ctx.pool)
        .await
        .map_err(err)?
        .into_iter()
        .collect())
}

#[tauri::command]
pub async fn settings_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    core::repo::settings_set(&state.ctx.pool, &key, &value)
        .await
        .map_err(err)
}
