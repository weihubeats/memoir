use memoir_core as core;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};

use core::ctx::Ctx;

fn mcp_err(e: anyhow::Error) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetContextParams {
    /// 用户本轮的问题/话题，用于检索相关历史
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub max_chars: Option<usize>,
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
pub struct MonthlyReportParams {
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub month: Option<u32>,
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
    /// normal / sensitive / secret；收入数额、持仓明细、健康异常、感情状况用 sensitive，卡号证件号等标识符用 secret
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

/// 个人上下文 MCP server：档案注入 / 混合检索 / 对话即采集 / 月报。
#[derive(Clone)]
pub struct MyContextServer {
    ctx: Ctx,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl MyContextServer {
    pub fn new(ctx: Ctx) -> Self {
        Self {
            ctx,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "获取注入上下文（每轮对话开始调用一次）：个人档案摘要 + 按话题检索的相关历史，总量受预算约束。query 传用户本轮的问题或话题。"
    )]
    async fn get_context(
        &self,
        Parameters(p): Parameters<GetContextParams>,
    ) -> Result<CallToolResult, McpError> {
        let text = core::context::get_context(
            &self.ctx,
            p.query.as_deref(),
            p.max_chars.unwrap_or(core::context::DEFAULT_BUDGET_CHARS),
        )
        .await
        .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[tool(
        description = "获取个人档案摘要：当前有效事实，按域分组的紧凑视图。默认不含 sensitive 级；secret 级永远不返回。"
    )]
    async fn get_profile(
        &self,
        Parameters(p): Parameters<GetProfileParams>,
    ) -> Result<CallToolResult, McpError> {
        let text = core::profile::get_profile(
            &self.ctx,
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

    #[tool(description = "按关键词+语义混合检索个人历史记录（日记/事件/对话摘录）。")]
    async fn search_entries(
        &self,
        Parameters(p): Parameters<SearchEntriesParams>,
    ) -> Result<CallToolResult, McpError> {
        let entries = core::search::search_entries(
            &self.ctx,
            &p.query,
            p.top_k.unwrap_or(core::search::DEFAULT_TOP_K),
            core::search::DEFAULT_MAX_CHARS,
        )
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
        let entries = core::repo::recent_entries(&self.ctx, p.limit.unwrap_or(10))
            .await
            .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            render_entries(&entries),
        )]))
    }

    #[tool(
        description = "生成本月（或指定月份）上下文月报材料：本月记录、待确认事实、操作统计。请基于它向用户做月度回顾。"
    )]
    async fn get_monthly_report(
        &self,
        Parameters(p): Parameters<MonthlyReportParams>,
    ) -> Result<CallToolResult, McpError> {
        let (year, month) = core::report::current_month();
        let text = core::report::monthly_report(
            &self.ctx,
            p.year.unwrap_or(year),
            p.month.unwrap_or(month),
        )
        .await
        .map_err(mcp_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[tool(
        description = "追加一条个人记录（日记/事件/对话摘录）。entities 传相关实体名，不存在的会自动创建。"
    )]
    async fn add_entry(
        &self,
        Parameters(p): Parameters<AddEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let entry = core::repo::add_entry(
            &self.ctx,
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
        description = "更新一条个人事实（如身高、就职公司、持仓、父母情况）。旧值自动封口保留历史。用户提到持久的个人信息时调用；一次性事件用 add_entry。"
    )]
    async fn upsert_fact(
        &self,
        Parameters(p): Parameters<UpsertFactParams>,
    ) -> Result<CallToolResult, McpError> {
        let (new_fact, superseded) = core::repo::upsert_fact(
            &self.ctx,
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
            &self.ctx,
            &p.name,
            p.kind.as_deref().unwrap_or("topic"),
            p.domain.as_deref(),
            "ai",
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
                Implementation::new("memoir", env!("CARGO_PKG_VERSION"))
                    .with_title("Memoir 个人上下文中枢"),
            )
            .with_instructions(
                "这是用户的个人上下文库（健康/财务/职业/家庭/偏好等，敏感分级 normal/sensitive/secret）。\
                 使用规则：\
                 1) 每轮对话开始调用 get_context（query 传用户本轮问题）获取档案与相关历史再回答；\
                 2) 用户提到持久的个人信息（身高体重、就职、收入资产、家人情况、偏好目标）→ upsert_fact；\
                 一次性事件与感受 → add_entry；\
                 3) sensitivity 默认 normal；收入数额、持仓明细、健康异常、感情状况标 sensitive；\
                 卡号/证件号/账号密码等标识符标 secret；\
                 4) 给建议时引用具体事实与记录，档案里没有的不要臆测。",
            )
    }
}

/// 以 stdio 运行 MCP server（CLI `serve` 子命令的入口）。
pub async fn serve_stdio(ctx: Ctx) -> anyhow::Result<()> {
    let service = MyContextServer::new(ctx)
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
