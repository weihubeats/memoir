use std::sync::Arc;

use async_trait::async_trait;
use my_context_core as core;
use serde_json::json;

/// 确定性 mock：按关键词映射到固定向量，验证向量通道与 RRF 融合。
struct MockEmbedder;

#[async_trait]
impl core::embed::Embedder for MockEmbedder {
    fn name(&self) -> &str {
        "mock"
    }
    async fn embed(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts
            .into_iter()
            .map(|t| {
                if ["跑", "健身", "运动", "锻炼"].iter().any(|k| t.contains(k)) {
                    vec![1.0, 0.0, 0.0, 0.0]
                } else if ["股票", "投资", "理财"].iter().any(|k| t.contains(k)) {
                    vec![0.0, 1.0, 0.0, 0.0]
                } else {
                    // 未命中关键词的文本给零向量：余弦为 0，不参与向量召回
                    vec![0.0, 0.0, 0.0, 0.0]
                }
            })
            .collect())
    }
}

async fn test_ctx() -> (core::Ctx, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = core::db::open_pool(dir.path().join("test.db").as_path())
        .await
        .expect("open pool");
    core::repo::seed_self_entity(&pool).await.expect("seed");
    (
        core::Ctx::from_parts(pool, Some(Arc::new(MockEmbedder))),
        dir,
    )
}

async fn add_entry(ctx: &core::Ctx, title: &str, body: &str) -> i64 {
    core::repo::add_entry(
        ctx,
        core::model::NewEntry {
            kind: "note".into(),
            title: title.into(),
            body: body.into(),
            tags: vec![],
            entity_names: vec![],
            occurred_at: None,
        },
        "test",
    )
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn fts_hits_long_query_and_like_falls_back_for_short() {
    let (ctx, _dir) = test_ctx().await;
    let a = add_entry(&ctx, "年度体检", "年度体检报告显示血脂偏高，需要复查。").await;
    let b = add_entry(&ctx, "开了股票账户", "在券商开了股票账户，开始定投。").await;

    // ≥3 字符走 FTS5 trigram
    let hits = core::search::search_entries(&ctx, "体检报告", 5, 10_000)
        .await
        .unwrap();
    assert_eq!(hits.iter().map(|e| e.id).collect::<Vec<_>>(), vec![a]);

    // <3 字符回退 LIKE
    let hits = core::search::search_entries(&ctx, "股票", 5, 10_000)
        .await
        .unwrap();
    assert_eq!(hits.iter().map(|e| e.id).collect::<Vec<_>>(), vec![b]);

    // 无关查询两边都搜不到
    let hits = core::search::search_entries(&ctx, "量子力学", 5, 10_000)
        .await
        .unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn vector_channel_finds_semantic_match_without_keyword_overlap() {
    let (ctx, _dir) = test_ctx().await;
    let a = add_entry(&ctx, "开始健身", "坚持跑步和健身三个月了，状态不错。").await;
    let _b = add_entry(&ctx, "研究投资", "看了看股票和基金，先小额试水。").await;

    // 「运动锻炼」与两条记录都无 3 字符子串重叠，只能靠向量通道命中 a
    let hits = core::search::search_entries(&ctx, "运动锻炼", 5, 10_000)
        .await
        .unwrap();
    assert_eq!(hits.first().map(|e| e.id), Some(a));
}

#[tokio::test]
async fn budget_trims_results() {
    let (ctx, _dir) = test_ctx().await;
    let long_body = "很长的记录内容。".repeat(40); // ~320 字
    let _a = add_entry(&ctx, "记录一", &long_body).await;
    let _b = add_entry(&ctx, "记录二", &long_body).await;
    let _c = add_entry(&ctx, "记录三", &long_body).await;

    let hits = core::search::search_entries(&ctx, "记录内容", 10, 400)
        .await
        .unwrap();
    assert!(
        hits.len() < 3,
        "预算 400 字最多放下一条 320 字的记录，实际 {}",
        hits.len()
    );
}

#[tokio::test]
async fn get_context_contains_profile_and_history() {
    let (ctx, _dir) = test_ctx().await;
    core::repo::upsert_fact(
        &ctx,
        core::model::FactUpsert {
            entity_name: "我".into(),
            key: "height".into(),
            value: json!("175cm"),
            domain: Some("health".into()),
            sensitivity: "normal".into(),
            source_entry_id: None,
        },
    )
    .await
    .unwrap();
    add_entry(&ctx, "年度体检", "体检报告显示血脂偏高。").await;

    let text = core::context::get_context(&ctx, Some("体检报告"), 10_000)
        .await
        .unwrap();
    assert!(text.contains("## 个人档案"));
    assert!(text.contains("175cm"));
    assert!(text.contains("## 相关历史"));
    assert!(text.contains("年度体检"));
}

#[tokio::test]
async fn stale_facts_and_monthly_report() {
    let (ctx, _dir) = test_ctx().await;
    core::repo::upsert_fact(
        &ctx,
        core::model::FactUpsert {
            entity_name: "我".into(),
            key: "weight".into(),
            value: json!("70kg"),
            domain: Some("health".into()),
            sensitivity: "normal".into(),
            source_entry_id: None,
        },
    )
    .await
    .unwrap();
    // 把更新时间拨回两年前，制造 stale
    sqlx::query("UPDATE facts SET updated_at = '2024-01-01T00:00:00Z'")
        .execute(&ctx.pool)
        .await
        .unwrap();
    add_entry(&ctx, "十月跑步", "本月开始每天跑步三公里。").await;

    let stale = core::report::list_stale_facts(&ctx.pool, 6).await.unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].1, "weight");

    let (year, month) = core::report::current_month();
    let report = core::report::monthly_report(&ctx, year, month)
        .await
        .unwrap();
    assert!(report.contains("十月跑步"), "月报含本月记录");
    assert!(report.contains("待确认事实"));
    assert!(report.contains("weight"), "月报列出过期事实");
    assert!(report.contains("add_entry"), "月报含操作统计");
}
