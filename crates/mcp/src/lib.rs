use my_context_core as core;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

fn mcp_err(e: anyhow::Error) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetProfileParams {
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    #[serde(default)]
    pub include_sensitive: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchEntriesParams {
    pub query: String,
    #[serde(default)]
    pub top_k: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecentEntriesParams {
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AddEntryParams {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub entities: Option<Vec<String>>,
    #[serde(default)]
    pub occurred_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpsertFactParams {
    pub entity: String,
    pub key: String,
    /// 任意 JSON 值或纯文本
    pub value: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub sensitivity: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AddEntityParams {
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

/// 个人上下文 MCP server：档案读取 / 记录检索 / 对话即采集。
#[derive(Clone)]
pub struct MyContextServer {
    pool: SqlitePool,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl MyContextServer {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "获取个人档案摘要：当前有效事实，按域分组的紧凑视图。默认不含 sensitive 级；secret 级永远不返回。"
    )]
    async fn get_profile(
        &self,
        Parameters(p): Parameters<GetProfileParams>,
    ) -> Result<CallToolResult, McpError> {
        let text = core::profile::get_profile(
            &self.pool,
            core::profile::ProfileOptions {
                domains: p.domains,
                include_sensitive: p.include_sensitive.unwrap_or(false),
                ..Default::default()
            },
        )
        .await
        .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[tool(description = "按关键词检索个人历史记录（日记/事件/对话摘录）。")]
    async fn search_entries(
        &self,
        Parameters(p): Parameters<SearchEntriesParams>,
    ) -> Result<CallToolResult, McpError> {
        let entries = core::repo::search_entries(&self.pool, &p.query, p.top_k.unwrap_or(10))
            .await
            .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            render_entries(&entries),
        )]))
    }

    #[tool(description = "获取最近的个人记录。")]
    async fn recent_entries(
        &self,
        Parameters(p): Parameters<RecentEntriesParams>,
    ) -> Result<CallToolResult, McpError> {
        let entries = core::repo::recent_entries(&self.pool, p.limit.unwrap_or(10))
            .await
            .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            render_entries(&entries),
        )]))
    }

    #[tool(
        description = "追加一条个人记录（日记/事件/对话摘录）。entities 传相关实体名，不存在的会自动创建。"
    )]
    async fn add_entry(
        &self,
        Parameters(p): Parameters<AddEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let entry = core::repo::add_entry(
            &self.pool,
            core::model::NewEntry {
                kind: p.kind.unwrap_or_else(|| "chat".to_owned()),
                title: p.title,
                body: p.body,
                tags: p.tags.unwrap_or_default(),
                entity_names: p.entities.unwrap_or_default(),
                occurred_at: p.occurred_at,
            },
            "ai",
        )
        .await
        .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "已记录 #{}：{}",
            entry.id, entry.title
        ))]))
    }

    #[tool(
        description = "更新一条个人事实（如身高、就职公司、持仓）。旧值自动封口保留历史。用户提到新的个人信息时应调用此工具落库。"
    )]
    async fn upsert_fact(
        &self,
        Parameters(p): Parameters<UpsertFactParams>,
    ) -> Result<CallToolResult, McpError> {
        let (new_fact, superseded) = core::repo::upsert_fact(
            &self.pool,
            core::model::FactUpsert {
                entity_name: p.entity,
                key: p.key,
                value: core::model::coerce_value(&p.value),
                domain: p.domain,
                sensitivity: p.sensitivity.unwrap_or_else(|| "normal".to_owned()),
                source_entry_id: None,
            },
        )
        .await
        .map_err(mcp_err)?;
        let msg = if superseded.is_some() {
            format!(
                "事实已更新（旧值已封口保留）：#{} {}",
                new_fact.id, new_fact.key
            )
        } else {
            format!("事实已新增：#{} {}", new_fact.id, new_fact.key)
        };
        Ok(CallToolResult::success(vec![ContentBlock::text(msg)]))
    }

    #[tool(description = "新增一个实体（人物/组织/账户/资产等）。")]
    async fn add_entity(
        &self,
        Parameters(p): Parameters<AddEntityParams>,
    ) -> Result<CallToolResult, McpError> {
        let entity = core::repo::add_entity(
            &self.pool,
            &p.name,
            p.kind.as_deref().unwrap_or("topic"),
            p.domain.as_deref(),
        )
        .await
        .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "实体已创建：{}（#{}, {}）",
            entity.name, entity.id, entity.kind
        ))]))
    }
}

fn render_entries(entries: &[core::model::Entry]) -> String {
    if entries.is_empty() {
        return "（无匹配记录）".to_owned();
    }
    entries
        .iter()
        .map(|e| {
            format!(
                "#{} [{}] {} — {}\n{}",
                e.id, e.kind, e.occurred_at, e.title, e.body
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MyContextServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("my-context", env!("CARGO_PKG_VERSION"))
                    .with_title("my-context 个人上下文中枢"),
            )
            .with_instructions(
                "这是用户的个人上下文库。回答前先 get_profile 了解用户；需要历史细节用 search_entries / recent_entries；用户提到新的个人信息时用 upsert_fact / add_entry 落库。",
            )
    }
}

/// 以 stdio 运行 MCP server（CLI `serve` 子命令的入口）。
pub async fn serve_stdio(pool: SqlitePool) -> anyhow::Result<()> {
    let service = MyContextServer::new(pool)
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
