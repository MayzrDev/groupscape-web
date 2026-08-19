use chrono::{Duration, Utc};
use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod};
use std::env;
use tokio_postgres::NoTls;

use server::config::Config;
use server::db;
use server::models::{DeathEvent, DialogueEvent, GameEvent, KillEvent, LootItem, ObjectInteractionEvent};

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

fn sample_kill() -> GameEvent {
    GameEvent::Kill(KillEvent {
        npc_id: 3129,
        npc_name: "Zulrah".to_string(),
        world_x: 2200,
        world_y: 3057,
        plane: 0,
        world: 420,
        loot: Some(vec![LootItem {
            item_id: 12934,
            quantity: 1,
        }]),
    })
}

fn sample_death() -> GameEvent {
    GameEvent::Death(DeathEvent {
        world_x: 3200,
        world_y: 3200,
        plane: 0,
        world: 420,
        killer_name: Some("Zulrah".to_string()),
    })
}

#[tokio::test]
async fn test_ensure_open_session_is_idempotent_and_atomic() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "sessiontest1").await;

    let first = db::ensure_open_session(&client, group_id)
        .await
        .expect("first ensure should succeed");
    let second = db::ensure_open_session(&client, group_id)
        .await
        .expect("second ensure should succeed");
    assert_eq!(first, second, "heartbeats within the same window reuse the open session");

    let sessions = db::list_sessions(&client, group_id, 10)
        .await
        .expect("query failed");
    assert_eq!(sessions.len(), 1, "only one open session should exist per group");
    assert!(sessions[0].ended_at.is_none());
}

#[tokio::test]
async fn test_ensure_open_session_scopes_per_group() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_a = create_test_group(&client, "sessiontest2a").await;
    let group_b = create_test_group(&client, "sessiontest2b").await;

    let session_a = db::ensure_open_session(&client, group_a).await.unwrap();
    let session_b = db::ensure_open_session(&client, group_b).await.unwrap();
    assert_ne!(session_a, session_b);
}

#[tokio::test]
async fn test_close_idle_sessions_closes_only_past_the_cutoff() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let fresh_group = create_test_group(&client, "sessiontest3fresh").await;
    let stale_group = create_test_group(&client, "sessiontest3stale").await;

    db::ensure_open_session(&client, fresh_group).await.unwrap();
    let stale_session = db::ensure_open_session(&client, stale_group).await.unwrap();
    client
        .execute(
            "UPDATE groupscape.sessions SET last_seen_at = $1 WHERE session_id = $2",
            &[&(Utc::now() - Duration::minutes(20)), &stale_session],
        )
        .await
        .unwrap();

    let closed = db::close_idle_sessions(&client, Duration::minutes(15))
        .await
        .expect("close should succeed");
    assert_eq!(closed, 1);

    let fresh_sessions = db::list_sessions(&client, fresh_group, 10).await.unwrap();
    assert!(fresh_sessions[0].ended_at.is_none(), "fresh session should stay open");

    let stale_sessions = db::list_sessions(&client, stale_group, 10).await.unwrap();
    assert!(stale_sessions[0].ended_at.is_some(), "stale session should be closed");

    // A heartbeat after close opens a brand new session rather than reusing the closed one.
    let reopened = db::ensure_open_session(&client, stale_group).await.unwrap();
    assert_ne!(reopened, stale_session);
}

#[tokio::test]
async fn test_insert_and_list_activity_events() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "activitytest1").await;
    let session_id = db::ensure_open_session(&client, group_id).await.unwrap();

    db::insert_activity_event(&client, group_id, session_id, "Zezima", &sample_kill())
        .await
        .expect("insert should succeed");
    db::insert_activity_event(&client, group_id, session_id, "Zezima", &sample_death())
        .await
        .expect("insert should succeed");

    let events = db::list_activity_events(&client, group_id, None, None, None, 30)
        .await
        .expect("query failed");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "death", "newest event sorts first");
    assert_eq!(events[1].event_type, "kill");
    assert_eq!(events[0].member_name, "Zezima");
    assert_eq!(events[0].session_id, session_id);
    assert_eq!(
        events[1].payload["npcName"],
        serde_json::json!("Zulrah"),
        "payload should round-trip the plugin's exact field names"
    );
}

#[tokio::test]
async fn test_list_activity_events_filters_by_member_and_type() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "activitytest2").await;
    let session_id = db::ensure_open_session(&client, group_id).await.unwrap();

    db::insert_activity_event(&client, group_id, session_id, "Zezima", &sample_kill())
        .await
        .unwrap();
    db::insert_activity_event(&client, group_id, session_id, "Woox", &sample_death())
        .await
        .unwrap();

    let zezima_events = db::list_activity_events(&client, group_id, Some("Zezima"), None, None, 30)
        .await
        .unwrap();
    assert_eq!(zezima_events.len(), 1);
    assert_eq!(zezima_events[0].member_name, "Zezima");

    let kill_events = db::list_activity_events(&client, group_id, None, Some("kill"), None, 30)
        .await
        .unwrap();
    assert_eq!(kill_events.len(), 1);
    assert_eq!(kill_events[0].event_type, "kill");
}

#[tokio::test]
async fn test_list_activity_events_is_scoped_per_group() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_a = create_test_group(&client, "activitytest3a").await;
    let group_b = create_test_group(&client, "activitytest3b").await;
    let session_a = db::ensure_open_session(&client, group_a).await.unwrap();

    db::insert_activity_event(&client, group_a, session_a, "Zezima", &sample_kill())
        .await
        .unwrap();

    let group_b_events = db::list_activity_events(&client, group_b, None, None, None, 30)
        .await
        .unwrap();
    assert!(group_b_events.is_empty());
}

#[tokio::test]
async fn test_kill_event_without_loot_round_trips() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "activitytest4").await;
    let session_id = db::ensure_open_session(&client, group_id).await.unwrap();

    let kill_without_loot = GameEvent::Kill(KillEvent {
        npc_id: 50,
        npc_name: "Giant rat".to_string(),
        world_x: 0,
        world_y: 0,
        plane: 0,
        world: 301,
        loot: None,
    });
    db::insert_activity_event(&client, group_id, session_id, "Zezima", &kill_without_loot)
        .await
        .expect("insert should succeed even when the plugin never correlated loot");

    let events = db::list_activity_events(&client, group_id, None, None, None, 30)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert!(events[0].payload.get("loot").is_none());
}

#[tokio::test]
async fn test_insert_dialogue_event_round_trips() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "activitytest5").await;
    let session_id = db::ensure_open_session(&client, group_id).await.unwrap();

    let dialogue = DialogueEvent {
        event_type: "dialogue".to_string(),
        npc_id: 941,
        npc_name: "Hans".to_string(),
        combat_level: 0,
        world_x: 3222,
        world_y: 3218,
        plane: 0,
        world: 301,
    };
    db::insert_dialogue_event(&client, group_id, session_id, "Zezima", &dialogue)
        .await
        .expect("insert should succeed");

    let events = db::list_activity_events(&client, group_id, None, None, None, 30)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "dialogue");
    assert_eq!(events[0].member_name, "Zezima");
    assert_eq!(events[0].payload["npcName"], serde_json::json!("Hans"));
}

#[tokio::test]
async fn test_insert_object_interaction_event_round_trips() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "activitytest6").await;
    let session_id = db::ensure_open_session(&client, group_id).await.unwrap();

    let object_interaction = ObjectInteractionEvent {
        object_id: 409,
        object_name: Some("Bank booth".to_string()),
        action: Some("Bank".to_string()),
        world_x: 3185,
        world_y: 3437,
        plane: 0,
        world: 301,
    };
    db::insert_object_interaction_event(&client, group_id, session_id, "Zezima", &object_interaction)
        .await
        .expect("insert should succeed");

    let events = db::list_activity_events(&client, group_id, None, None, None, 30)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "object_interaction");
    assert_eq!(events[0].payload["objectName"], serde_json::json!("Bank booth"));
}
