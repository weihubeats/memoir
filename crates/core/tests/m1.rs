use my_context_core as core;
use serde_json::json;

async fn test_pool() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = core::db::open_pool(dir.path().join("test.db").as_path())
        .await
        .expect("open pool");
    core::repo::seed_self_entity(&pool).await.expect("seed");
    (pool, dir)
}

#[tokio::test]
async fn fact_versioning_keeps_history_and_profile_reflects_latest() {
    let (pool, _dir) = test_pool().await;
    for value in ["175cm", "176cm"] {
        core::repo::upsert_fact(
            &pool,
            core::model::FactUpsert {
                entity_name: "我".into(),
                key: "height".into(),
                value: json!(value),
                domain: Some("health".into()),
                sensitivity: "normal".into(),
                source_entry_id: None,
            },
        )
        .await
        .unwrap();
    }

    let history = core::repo::list_facts(&pool, None, false).await.unwrap();
    assert_eq!(history.len(), 2);
    assert!(
        history.iter().any(|f| f.valid_to.is_some()),
        "旧值应封口保留"
    );

    let profile = core::profile::get_profile(&pool, core::profile::ProfileOptions::default())
        .await
        .unwrap();
    assert!(profile.contains("176cm"));
    assert!(!profile.contains("175cm"), "旧值不应出现在档案里");
}

#[tokio::test]
async fn profile_respects_sensitivity() {
    let (pool, _dir) = test_pool().await;
    for (key, value, sens) in [
        ("diet", json!("清淡"), "normal"),
        ("salary", json!("50万"), "sensitive"),
        ("card_no", json!("6222..."), "secret"),
    ] {
        core::repo::upsert_fact(
            &pool,
            core::model::FactUpsert {
                entity_name: "我".into(),
                key: key.into(),
                value,
                domain: Some("wealth".into()),
                sensitivity: sens.into(),
                source_entry_id: None,
            },
        )
        .await
        .unwrap();
    }
    let default = core::profile::get_profile(&pool, core::profile::ProfileOptions::default())
        .await
        .unwrap();
    assert!(default.contains("diet"));
    assert!(!default.contains("salary"), "sensitive 级默认不进档案");
    assert!(!default.contains("card_no"), "secret 级默认不进档案");

    let with_sensitive = core::profile::get_profile(
        &pool,
        core::profile::ProfileOptions {
            include_sensitive: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(with_sensitive.contains("salary"));
    assert!(!with_sensitive.contains("card_no"), "secret 级永远不进档案");
}

#[tokio::test]
async fn entries_search_and_audit() {
    let (pool, _dir) = test_pool().await;
    let entry = core::repo::add_entry(
        &pool,
        core::model::NewEntry {
            kind: "event".into(),
            title: "体检完成".into(),
            body: "血脂偏高，医生建议控制饮食。".into(),
            tags: vec!["体检".into()],
            entity_names: vec!["我".into()],
            occurred_at: None,
        },
        "test",
    )
    .await
    .unwrap();
    assert!(!entry.entity_ids.is_empty(), "应自动关联实体");

    let hits = core::repo::search_entries(&pool, "血脂", 5).await.unwrap();
    assert_eq!(hits.len(), 1);
    let misses = core::repo::search_entries(&pool, "不存在的词", 5)
        .await
        .unwrap();
    assert!(misses.is_empty());

    core::repo::upsert_fact(
        &pool,
        core::model::FactUpsert {
            entity_name: "我".into(),
            key: "sleep".into(),
            value: json!("23:30"),
            domain: Some("health".into()),
            sensitivity: "normal".into(),
            source_entry_id: None,
        },
    )
    .await
    .unwrap();
    let audit = core::repo::list_audit(&pool, 10).await.unwrap();
    assert!(audit.iter().any(|a| a.action == "add_entry"));
    assert!(audit.iter().any(|a| a.action == "upsert_fact"));
}

#[tokio::test]
async fn export_contains_all_data() {
    let (pool, _dir) = test_pool().await;
    core::repo::upsert_fact(
        &pool,
        core::model::FactUpsert {
            entity_name: "我".into(),
            key: "height".into(),
            value: json!("175cm"),
            domain: Some("health".into()),
            sensitivity: "secret".into(),
            source_entry_id: None,
        },
    )
    .await
    .unwrap();
    let doc = core::export::export_json(&pool).await.unwrap();
    assert_eq!(doc["facts"].as_array().unwrap().len(), 1);
    let md = core::export::export_markdown(&pool).await.unwrap();
    assert!(md.contains("175cm"), "导出含 secret 级（数据主权兜底）");
}
