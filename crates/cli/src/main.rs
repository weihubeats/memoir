use std::path::PathBuf;

use clap::{Parser, Subcommand};
use my_context_core as core;

#[derive(Parser)]
#[command(
    name = "my-context",
    version,
    about = "个人上下文中枢：把个人的所有信息存一处，供 AI 读取"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 初始化数据库（建目录、跑迁移、种子实体）
    Init,
    /// 以 MCP stdio server 运行，供 AI 客户端接入
    Serve,
    /// 录入数据
    Add {
        #[command(subcommand)]
        kind: AddKind,
    },
    /// 查看数据
    List {
        #[command(subcommand)]
        kind: ListKind,
    },
    /// 导出全库（数据主权兜底）
    Export {
        /// json 或 markdown
        #[arg(long, default_value = "markdown")]
        format: String,
        /// 输出文件（缺省打印到 stdout）
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum AddKind {
    /// 新增实体
    Entity {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "topic")]
        kind: String,
        #[arg(long)]
        domain: Option<String>,
    },
    /// 更新事实（旧值封口保留历史）
    Fact {
        #[arg(long)]
        entity: String,
        #[arg(long)]
        key: String,
        /// 任意文本，能解析成 JSON 就按 JSON 存
        #[arg(long)]
        value: String,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long, default_value = "normal")]
        sensitivity: String,
    },
    /// 追加一条记录
    Entry {
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "")]
        body: String,
        #[arg(long, default_value = "note")]
        kind: String,
        /// 逗号分隔
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// 逗号分隔的实体名
        #[arg(long, value_delimiter = ',')]
        entities: Vec<String>,
        /// RFC3339 时间，缺省为现在
        #[arg(long)]
        occurred_at: Option<String>,
    },
}

#[derive(Subcommand)]
enum ListKind {
    Entities,
    Facts {
        #[arg(long)]
        entity: Option<String>,
        /// 只看当前有效（默认 true；--active=false 时含历史）
        #[arg(long, default_value_t = true)]
        active: bool,
    },
    Entries {
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let db = core::db::default_db_path()?;

    match cli.command {
        Command::Init => {
            let pool = core::db::ensure_pool(&db).await?;
            pool.close().await;
            println!("已初始化：{}", db.display());
        }
        Command::Serve => {
            let pool = core::db::ensure_pool(&db).await?;
            my_context_mcp::serve_stdio(pool).await?;
        }
        Command::Add { kind } => {
            let pool = core::db::ensure_pool(&db).await?;
            match kind {
                AddKind::Entity { name, kind, domain } => {
                    let e = core::repo::add_entity(&pool, &name, &kind, domain.as_deref()).await?;
                    println!("实体已创建：{}（#{}, {}）", e.name, e.id, e.kind);
                }
                AddKind::Fact {
                    entity,
                    key,
                    value,
                    domain,
                    sensitivity,
                } => {
                    let (f, superseded) = core::repo::upsert_fact(
                        &pool,
                        core::model::FactUpsert {
                            entity_name: entity,
                            key,
                            value: core::model::coerce_value(&value),
                            domain,
                            sensitivity,
                            source_entry_id: None,
                        },
                    )
                    .await?;
                    println!(
                        "{}：#{}  {} = {}",
                        if superseded.is_some() {
                            "事实已更新（旧值封口）"
                        } else {
                            "事实已新增"
                        },
                        f.id,
                        f.key,
                        f.value
                    );
                }
                AddKind::Entry {
                    title,
                    body,
                    kind,
                    tags,
                    entities,
                    occurred_at,
                } => {
                    let e = core::repo::add_entry(
                        &pool,
                        core::model::NewEntry {
                            kind,
                            title,
                            body,
                            tags,
                            entity_names: entities,
                            occurred_at,
                        },
                        "cli",
                    )
                    .await?;
                    println!("记录已写入：#{} {}", e.id, e.title);
                }
            }
            pool.close().await;
        }
        Command::List { kind } => {
            let pool = core::db::ensure_pool(&db).await?;
            match kind {
                ListKind::Entities => {
                    for e in core::repo::list_entities(&pool).await? {
                        println!(
                            "#{} [{}] {}  domain={} status={}",
                            e.id,
                            e.kind,
                            e.name,
                            e.domain.as_deref().unwrap_or("-"),
                            e.status
                        );
                    }
                }
                ListKind::Facts { entity, active } => {
                    let entity_id = match entity.as_deref() {
                        Some(name) => Some(
                            core::repo::find_entity_by_name(&pool, name)
                                .await?
                                .ok_or_else(|| anyhow::anyhow!("实体不存在：{name}"))?
                                .id,
                        ),
                        None => None,
                    };
                    for f in core::repo::list_facts(&pool, entity_id, active).await? {
                        println!(
                            "#{} entity#{} [{}] {} = {}  自 {}{}",
                            f.id,
                            f.entity_id,
                            f.sensitivity,
                            f.key,
                            f.value,
                            f.valid_from,
                            match &f.valid_to {
                                Some(t) => format!("  至 {t}（历史）"),
                                None => String::new(),
                            }
                        );
                    }
                }
                ListKind::Entries { limit } => {
                    for e in core::repo::recent_entries(&pool, limit).await? {
                        println!("#{} [{}] {}  {}", e.id, e.kind, e.occurred_at, e.title);
                        if !e.body.is_empty() {
                            println!("    {}", e.body.replace('\n', "\n    "));
                        }
                    }
                }
            }
            pool.close().await;
        }
        Command::Export { format, out } => {
            let pool = core::db::ensure_pool(&db).await?;
            let text = match format.as_str() {
                "json" => serde_json::to_string_pretty(&core::export::export_json(&pool).await?)?,
                "markdown" | "md" => core::export::export_markdown(&pool).await?,
                other => anyhow::bail!("不支持的格式：{other}（可选 json / markdown）"),
            };
            match out {
                Some(path) => {
                    std::fs::write(&path, text)?;
                    println!("已导出到 {}", path.display());
                }
                None => print!("{text}"),
            }
            pool.close().await;
        }
    }
    Ok(())
}
