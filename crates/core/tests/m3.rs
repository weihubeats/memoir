use my_context_core as core;
use serde_json::json;

async fn test_ctx() -> (core::Ctx, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = core::db::open_pool(dir.path().join("test.db").as_path())
        .await
        .expect("open pool");
    core::repo::seed_self_entity(&pool).await.expect("seed");
    (core::Ctx::from_parts(pool, None), dir)
}

async fn add_entry(ctx: &core::Ctx, title: &str, entities: &[&str]) -> i64 {
    core::repo::add_entry(
        ctx,
        core::model::NewEntry {
            kind: "note".into(),
            title: title.into(),
            body: String::new(),
            tags: vec![],
            entity_names: entities.iter().map(|s| s.to_string()).collect(),
            occurred_at: None,
        },
        "test",
    )
    .await
    .unwrap()
    .id
}

async fn upsert(ctx: &core::Ctx, entity: &str, key: &str, value: &str) -> i64 {
    core::repo::upsert_fact(
        ctx,
        core::model::FactUpsert {
            entity_name: entity.into(),
            key: key.into(),
            value: json!(value),
            domain: Some("health".into()),
            sensitivity: "normal".into(),
            source_entry_id: None,
        },
    )
    .await
    .unwrap()
    .0
    .id
}

#[tokio::test]
async fn entry_trash_cycle() {
    let (ctx, _dir) = test_ctx().await;
    let id = add_entry(&ctx, "待删记录", &[]).await;

    core::repo::delete_entry(&ctx, id, "test").await.unwrap();
    assert!(core::repo::recent_entries(&ctx, 10)
        .await
        .unwrap()
        .is_empty());
    let trash = core::repo::list_trash(&ctx).await.unwrap();
    assert_eq!(trash.entries.len(), 1);

    core::repo::restore_entry(&ctx, id, "test").await.unwrap();
    assert_eq!(core::repo::recent_entries(&ctx, 10).await.unwrap().len(), 1);
    assert!(core::repo::list_trash(&ctx)
        .await
        .unwrap()
        .entries
        .is_empty());

    core::repo::delete_entry(&ctx, id, "test").await.unwrap();
    core::repo::purge_entry(&ctx, id, "test").await.unwrap();
    assert!(core::repo::get_entry(&ctx, id).await.unwrap().is_none());
}

#[tokio::test]
async fn fact_delete_restore_and_conflict() {
    let (ctx, _dir) = test_ctx().await;
    let id = upsert(&ctx, "我", "height", "175cm").await;

    core::repo::delete_fact(&ctx, id, "test").await.unwrap();
    assert!(core::repo::list_facts(&ctx, None, true)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(core::repo::list_trash(&ctx).await.unwrap().facts.len(), 1);

    core::repo::restore_fact(&ctx, id, "test").await.unwrap();
    assert_eq!(
        core::repo::list_facts(&ctx, None, true)
            .await
            .unwrap()
            .len(),
        1
    );

    // 删除后同键写了新事实 → 恢复旧事实应拒绝（双活）
    core::repo::delete_fact(&ctx, id, "test").await.unwrap();
    let _new = upsert(&ctx, "我", "height", "176cm").await;
    assert!(core::repo::restore_fact(&ctx, id, "test").await.is_err());

    // 彻底清除后彻底消失
    core::repo::purge_fact(&ctx, id, "test").await.unwrap();
    assert!(core::repo::get_fact(&ctx, id).await.is_err());
}

#[tokio::test]
async fn merge_entities_moves_and_resolves_conflicts() {
    let (ctx, _dir) = test_ctx().await;
    let a = core::repo::add_entity(&ctx, "张三", "person", None, "test")
        .await
        .unwrap()
        .id;
    let b = core::repo::add_entity(&ctx, "张三丰", "person", None, "test")
        .await
        .unwrap()
        .id;
    let _fa_shared = upsert(&ctx, "张三", "city", "北京").await;
    let _fa_unique = upsert(&ctx, "张三", "hobby", "书法").await;
    let _fb_shared = upsert(&ctx, "张三丰", "city", "上海").await; // 与 A 的 city 冲突
    let e1 = add_entry(&ctx, "和张三吃饭", &["张三"]).await;
    let _e2 = add_entry(&ctx, "和张三丰喝茶", &["张三丰"]).await;

    core::repo::merge_entities(&ctx, a, b, "test")
        .await
        .unwrap();

    // A 物理消失，B 保留
    assert!(core::repo::get_entity(&ctx, a).await.is_err());
    assert!(core::repo::get_entity(&ctx, b).await.is_ok());

    // 冲突键只剩一条有效（保留 B 的上海，较新）
    let facts = core::repo::list_facts(&ctx, Some(b), true).await.unwrap();
    let city: Vec<_> = facts.iter().filter(|f| f.key == "city").collect();
    assert_eq!(city.len(), 1);
    assert_eq!(city[0].value, "\"上海\"");
    assert!(
        facts.iter().any(|f| f.key == "hobby"),
        "A 的独有事实移了过来"
    );

    // 关联移到 B
    let entry = core::repo::get_entry(&ctx, e1).await.unwrap().unwrap();
    assert_eq!(entry.entity_ids, vec![b]);
}

#[tokio::test]
async fn auto_purge_removes_expired_trash() {
    let (ctx, _dir) = test_ctx().await;
    let id = add_entry(&ctx, "很久前的删除", &[]).await;
    core::repo::delete_entry(&ctx, id, "test").await.unwrap();
    sqlx::query("UPDATE entries SET deleted_at = '2020-01-01T00:00:00Z'")
        .execute(&ctx.pool)
        .await
        .unwrap();

    let (n_entries, _) = core::repo::auto_purge(&ctx.pool).await.unwrap();
    assert_eq!(n_entries, 1);
    assert!(core::repo::get_entry(&ctx, id).await.unwrap().is_none());
}

#[tokio::test]
async fn deleted_entity_hidden_and_restorable() {
    let (ctx, _dir) = test_ctx().await;
    let e = core::repo::add_entity(&ctx, "临时实体", "topic", None, "test")
        .await
        .unwrap();
    let _f = upsert(&ctx, "临时实体", "mood", "平静").await;

    core::repo::delete_entity(&ctx, e.id, "test").await.unwrap();
    // 档案与实体列表都不再出现（种子实体「我」仍在，只看临时实体）
    assert!(!core::repo::list_entities(&ctx)
        .await
        .unwrap()
        .iter()
        .any(|x| x.id == e.id));
    let profile = core::profile::get_profile(&ctx, core::profile::ProfileOptions::default())
        .await
        .unwrap();
    assert!(!profile.contains("mood"));

    core::repo::restore_entity(&ctx, e.id, "test")
        .await
        .unwrap();
    assert!(core::repo::list_entities(&ctx)
        .await
        .unwrap()
        .iter()
        .any(|x| x.id == e.id));
    // 事实随之回来
    assert_eq!(
        core::repo::list_facts(&ctx, None, true)
            .await
            .unwrap()
            .len(),
        1
    );
}
