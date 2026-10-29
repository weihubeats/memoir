use serde::Serialize;
use serde_json::Value;

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// 把自由文本值规范成 JSON：能解析就按 JSON 存，否则包成字符串。
pub fn coerce_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or(Value::String(raw.to_owned()))
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Entity {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub domain: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FactRow {
    pub id: i64,
    pub entity_id: i64,
    pub key: String,
    /// JSON 文本；展示/导出时用 [`FactRow::value_json`]
    pub value: String,
    pub domain: Option<String>,
    pub sensitivity: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub source_entry_id: Option<i64>,
    pub deleted_at: Option<String>,
}

impl FactRow {
    pub fn value_json(&self) -> Value {
        serde_json::from_str(&self.value).unwrap_or(Value::String(self.value.clone()))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub id: i64,
    pub occurred_at: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub entity_ids: Vec<i64>,
    pub created_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub at: String,
    pub action: String,
    pub object_type: String,
    pub object_id: Option<i64>,
    pub source: String,
    pub summary: String,
}

/// `upsert_fact` 入参。`domain` 缺省由档案层自行分组为 general。
pub struct FactUpsert {
    pub entity_name: String,
    pub key: String,
    pub value: Value,
    pub domain: Option<String>,
    pub sensitivity: String,
    pub source_entry_id: Option<i64>,
}

pub struct NewEntry {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    /// 按实体名关联；不存在的自动建为 topic 实体
    pub entity_names: Vec<String>,
    pub occurred_at: Option<String>,
}
