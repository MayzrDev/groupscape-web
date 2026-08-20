use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod};
use std::env;
use tokio_postgres::NoTls;

use server::config::Config;
use server::db;

/// Serializes integration tests since they all share the same database
/// and each test drops/recreates the schema.
static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn create_test_pool() -> Pool {
    let mut cfg = if let Ok(url) = env::var("TEST_DATABASE_URL") {
        let mut c = deadpool_postgres::Config::new();
        c.url = Some(url);
        c
    } else {
        let config = Config::from_env().expect("failed to read config.toml");
        let mut pg = config.pg.clone();
        pg.dbname = Some("groupscape_test".to_string());
        pg
    };

    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    cfg.create_pool(None, NoTls)
        .expect("failed to create test pool")
}

/// Fresh schema, migrated by the server's own `db::update_schema`.
async fn setup(pool: &Pool) {
    let mut client = pool.get().await.expect("failed to get client");
    client
        .execute("DROP SCHEMA IF EXISTS groupscape CASCADE", &[])
        .await
        .expect("failed to drop schema");
    client
        .execute("CREATE SCHEMA IF NOT EXISTS groupscape", &[])
        .await
        .expect("failed to create schema");
    client
        .execute(
            r#"CREATE TABLE groupscape.groups(
                group_id BIGSERIAL UNIQUE,
                group_name TEXT NOT NULL,
                group_token_hash CHAR(64) NOT NULL,
                PRIMARY KEY (group_name, group_token_hash)
            )"#,
            &[],
        )
        .await
        .expect("failed to create groups table");

    db::update_schema(&mut client)
        .await
        .expect("failed to update schema");
}

async fn create_test_group(client: &deadpool_postgres::Client, name: &str) -> i64 {
    let hashed_token = server::crypto::token_hash("test-token", name);
    let row = client
        .query_one(
            "INSERT INTO groupscape.groups (group_name, group_token_hash, version) VALUES ($1, $2, 2) RETURNING group_id",
            &[&name, &hashed_token],
        )
        .await
        .expect("failed to create test group");
    row.get(0)
}

async fn create_test_account(client: &deadpool_postgres::Client, email: &str) -> i64 {
    db::create_account(client, email, "not-a-real-hash")
        .await
        .expect("failed to create test account")
}

#[tokio::test]
async fn test_insert_and_list_group_goals() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "goalstest1").await;
    let account_id = create_test_account(&client, "goals1@example.com").await;

    let goal = db::insert_group_goal(
        &client,
        group_id,
        "Get a fire cape",
        "none",
        None,
        account_id,
    )
    .await
    .expect("insert should succeed");
    assert_eq!(goal.title, "Get a fire cape");
    assert_eq!(goal.status, "open");
    assert_eq!(goal.reference_type, "none");
    assert_eq!(goal.reference_id, None);
    assert_eq!(goal.created_by, account_id);
    assert!(!goal.auto_completed);

    let goals = db::list_group_goals(&client, group_id).await.unwrap();
    assert_eq!(goals.len(), 1);
    assert_eq!(goals[0].id, goal.id);
}

#[tokio::test]
async fn test_list_group_goals_scopes_per_group_and_orders_newest_first() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_a = create_test_group(&client, "goalstest2a").await;
    let group_b = create_test_group(&client, "goalstest2b").await;
    let account_id = create_test_account(&client, "goals2@example.com").await;

    db::insert_group_goal(&client, group_a, "Goal A1", "none", None, account_id)
        .await
        .unwrap();
    let a2 = db::insert_group_goal(&client, group_a, "Goal A2", "none", None, account_id)
        .await
        .unwrap();
    db::insert_group_goal(&client, group_b, "Goal B1", "none", None, account_id)
        .await
        .unwrap();

    let goals_a = db::list_group_goals(&client, group_a).await.unwrap();
    assert_eq!(goals_a.len(), 2);
    assert_eq!(goals_a[0].id, a2.id, "newest goal sorts first");

    let goals_b = db::list_group_goals(&client, group_b).await.unwrap();
    assert_eq!(goals_b.len(), 1);
    assert_eq!(goals_b[0].title, "Goal B1");
}

#[tokio::test]
async fn test_find_group_goal_scopes_to_group() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_a = create_test_group(&client, "goalstest3a").await;
    let group_b = create_test_group(&client, "goalstest3b").await;
    let account_id = create_test_account(&client, "goals3@example.com").await;

    let goal = db::insert_group_goal(&client, group_a, "Goal", "none", None, account_id)
        .await
        .unwrap();

    let found = db::find_group_goal(&client, group_a, goal.id)
        .await
        .unwrap();
    assert!(found.is_some());

    let not_found = db::find_group_goal(&client, group_b, goal.id)
        .await
        .unwrap();
    assert!(
        not_found.is_none(),
        "a goal from another group must not be findable"
    );
}

#[tokio::test]
async fn test_update_group_goal_patches_only_provided_fields() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "goalstest4").await;
    let account_id = create_test_account(&client, "goals4@example.com").await;

    let goal = db::insert_group_goal(
        &client,
        group_id,
        "Original",
        "skill",
        Some("slayer"),
        account_id,
    )
    .await
    .unwrap();

    let updated = db::update_group_goal(&client, group_id, goal.id, Some("Renamed"), None, None)
        .await
        .unwrap()
        .expect("goal should exist");
    assert_eq!(updated.title, "Renamed");
    assert_eq!(
        updated.reference_type, "skill",
        "untouched field stays as-is"
    );
    assert_eq!(updated.reference_id.as_deref(), Some("slayer"));
    assert!(updated.updated_at >= goal.updated_at);
}

#[tokio::test]
async fn test_update_group_goal_can_clear_reference_id() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "goalstest5").await;
    let account_id = create_test_account(&client, "goals5@example.com").await;

    let goal = db::insert_group_goal(
        &client,
        group_id,
        "Goal",
        "skill",
        Some("slayer"),
        account_id,
    )
    .await
    .unwrap();

    let updated = db::update_group_goal(&client, group_id, goal.id, None, Some("none"), Some(None))
        .await
        .unwrap()
        .expect("goal should exist");
    assert_eq!(updated.reference_type, "none");
    assert_eq!(updated.reference_id, None);
}

#[tokio::test]
async fn test_update_group_goal_returns_none_for_unknown_id() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "goalstest6").await;

    let updated = db::update_group_goal(&client, group_id, 999999, Some("x"), None, None)
        .await
        .unwrap();
    assert!(updated.is_none());
}

#[tokio::test]
async fn test_set_group_goal_status_completes_and_records_completer() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "goalstest7").await;
    let creator = create_test_account(&client, "goals7-creator@example.com").await;
    let completer = create_test_account(&client, "goals7-completer@example.com").await;

    let goal = db::insert_group_goal(&client, group_id, "Goal", "none", None, creator)
        .await
        .unwrap();

    let completed =
        db::set_group_goal_status(&client, group_id, goal.id, "complete", Some(completer))
            .await
            .unwrap()
            .expect("goal should exist");
    assert_eq!(completed.status, "complete");
    assert_eq!(completed.completed_by, Some(completer));
    assert!(
        !completed.auto_completed,
        "manual toggle is never auto_completed"
    );

    let reopened = db::set_group_goal_status(&client, group_id, goal.id, "open", None)
        .await
        .unwrap()
        .expect("goal should exist");
    assert_eq!(reopened.status, "open");
    assert_eq!(reopened.completed_by, None);
}

#[tokio::test]
async fn test_delete_group_goal_removes_it_and_reports_whether_it_existed() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "goalstest8").await;
    let account_id = create_test_account(&client, "goals8@example.com").await;

    let goal = db::insert_group_goal(&client, group_id, "Goal", "none", None, account_id)
        .await
        .unwrap();

    let deleted = db::delete_group_goal(&client, group_id, goal.id)
        .await
        .unwrap();
    assert!(deleted);

    let goals = db::list_group_goals(&client, group_id).await.unwrap();
    assert!(goals.is_empty());

    let deleted_again = db::delete_group_goal(&client, group_id, goal.id)
        .await
        .unwrap();
    assert!(
        !deleted_again,
        "deleting an already-gone goal reports false, not an error"
    );
}

#[tokio::test]
async fn test_group_goal_cascades_on_group_delete() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "goalstest9").await;
    let account_id = create_test_account(&client, "goals9@example.com").await;

    db::insert_group_goal(&client, group_id, "Goal", "none", None, account_id)
        .await
        .unwrap();

    client
        .execute(
            "DELETE FROM groupscape.groups WHERE group_id = $1",
            &[&group_id],
        )
        .await
        .expect("group delete should cascade to group_goals");

    let goals = db::list_group_goals(&client, group_id).await.unwrap();
    assert!(goals.is_empty());
}
