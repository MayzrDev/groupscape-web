use chrono::{Duration, Utc};
use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod};
use std::env;
use tokio_postgres::NoTls;

use server::config::Config;
use server::crypto;
use server::db;
use server::error::ApiError;

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

    cfg.create_pool(None, NoTls).expect("failed to create test pool")
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

#[tokio::test]
async fn test_create_and_find_account_by_email() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "player@example.com", &password_hash)
        .await
        .expect("failed to create account");

    let account = db::get_account_by_email(&client, "player@example.com")
        .await
        .expect("query failed")
        .expect("account should exist");

    assert_eq!(account.id, account_id);
    assert_eq!(account.email.as_deref(), Some("player@example.com"));
    assert_eq!(account.password_hash.as_deref(), Some(password_hash.as_str()));
    assert!(!account.disabled);
}

#[tokio::test]
async fn test_duplicate_email_is_rejected() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    db::create_account(&client, "dupe@example.com", &password_hash)
        .await
        .expect("first registration should succeed");

    let result = db::create_account(&client, "dupe@example.com", &password_hash).await;
    assert!(matches!(result, Err(ApiError::EmailAlreadyRegisteredError)));
}

#[tokio::test]
async fn test_email_uniqueness_is_case_insensitive() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    db::create_account(&client, "Case@Example.com", &password_hash)
        .await
        .expect("first registration should succeed");

    let result = db::create_account(&client, "case@example.com", &password_hash).await;
    assert!(matches!(result, Err(ApiError::EmailAlreadyRegisteredError)));

    let found = db::get_account_by_email(&client, "CASE@EXAMPLE.COM")
        .await
        .expect("query failed");
    assert!(found.is_some(), "email lookup should be case-insensitive");
}

#[tokio::test]
async fn test_session_token_round_trip() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "session@example.com", &password_hash)
        .await
        .unwrap();

    let token = crypto::new_session_token();
    let token_hash = crypto::session_token_hash(&token);
    let expires_at = Utc::now() + Duration::days(30);
    db::create_account_session(&client, account_id, &token_hash, &expires_at)
        .await
        .expect("failed to create session");

    let account = db::get_account_by_session_token_hash(&client, &token_hash)
        .await
        .expect("query failed")
        .expect("session should resolve to the account");
    assert_eq!(account.id, account_id);
    assert_eq!(account.email.as_deref(), Some("session@example.com"));

    let wrong_hash = crypto::session_token_hash("not-the-real-token");
    let missing = db::get_account_by_session_token_hash(&client, &wrong_hash)
        .await
        .expect("query failed");
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_expired_session_does_not_authenticate() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "expired@example.com", &password_hash)
        .await
        .unwrap();

    let token = crypto::new_session_token();
    let token_hash = crypto::session_token_hash(&token);
    let already_expired = Utc::now() - Duration::minutes(1);
    db::create_account_session(&client, account_id, &token_hash, &already_expired)
        .await
        .unwrap();

    let account = db::get_account_by_session_token_hash(&client, &token_hash)
        .await
        .expect("query failed");
    assert!(account.is_none(), "expired session should not authenticate");
}

#[tokio::test]
async fn test_disabled_account_session_does_not_authenticate() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "disabled@example.com", &password_hash)
        .await
        .unwrap();
    client
        .execute(
            "UPDATE groupscape.accounts SET disabled = true WHERE id = $1",
            &[&account_id],
        )
        .await
        .unwrap();

    let token = crypto::new_session_token();
    let token_hash = crypto::session_token_hash(&token);
    let expires_at = Utc::now() + Duration::days(30);
    db::create_account_session(&client, account_id, &token_hash, &expires_at)
        .await
        .unwrap();

    let account = db::get_account_by_session_token_hash(&client, &token_hash)
        .await
        .expect("query failed");
    assert!(account.is_none(), "disabled account should not authenticate");
}

#[tokio::test]
async fn test_create_and_find_account_by_discord_id() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let account_id = db::create_account_with_discord_id(&client, "111222333")
        .await
        .expect("failed to create discord account");

    let account = db::get_account_by_discord_id(&client, "111222333")
        .await
        .expect("query failed")
        .expect("account should exist");

    assert_eq!(account.id, account_id);
    assert_eq!(account.email, None, "discord-only account has no email");
    assert_eq!(account.password_hash, None);
    assert!(!account.disabled);
}

#[tokio::test]
async fn test_unknown_discord_id_returns_none() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let account = db::get_account_by_discord_id(&client, "does-not-exist")
        .await
        .expect("query failed");
    assert!(account.is_none());
}

#[tokio::test]
async fn test_discord_id_is_unique() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    db::create_account_with_discord_id(&client, "444555666")
        .await
        .expect("first link should succeed");

    let result = db::create_account_with_discord_id(&client, "444555666").await;
    assert!(result.is_err(), "same discord id should not link twice");
}

#[tokio::test]
async fn test_link_character_creates_new_character() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "linker@example.com", &password_hash)
        .await
        .unwrap();

    let character = db::create_character(&client, account_id, "hash-1", "Zezima")
        .await
        .expect("failed to create character");

    assert_eq!(character.account_id, account_id);
    assert_eq!(character.account_hash, "hash-1");
    assert_eq!(character.display_rsn, "Zezima");

    let found = db::find_character_by_account_hash(&client, "hash-1")
        .await
        .expect("query failed")
        .expect("character should exist");
    assert_eq!(found.id, character.id);

    let count = db::count_characters_for_account(&client, account_id)
        .await
        .expect("query failed");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_link_character_unknown_hash_returns_none() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let found = db::find_character_by_account_hash(&client, "does-not-exist")
        .await
        .expect("query failed");
    assert!(found.is_none());
}

#[tokio::test]
async fn test_relinking_same_account_refreshes_display_rsn() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "refresher@example.com", &password_hash)
        .await
        .unwrap();

    let character = db::create_character(&client, account_id, "hash-2", "OldName")
        .await
        .unwrap();

    let refreshed = db::update_character_display_rsn(&client, character.id, "NewName")
        .await
        .expect("failed to refresh display rsn");
    assert_eq!(refreshed.id, character.id);
    assert_eq!(refreshed.display_rsn, "NewName");
    assert_eq!(refreshed.account_hash, "hash-2");

    let count = db::count_characters_for_account(&client, account_id)
        .await
        .expect("query failed");
    assert_eq!(count, 1, "refresh should not create a second character");
}

#[tokio::test]
async fn test_account_hash_is_unique_across_accounts() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_a = db::create_account(&client, "a@example.com", &password_hash)
        .await
        .unwrap();
    let account_b = db::create_account(&client, "b@example.com", &password_hash)
        .await
        .unwrap();

    db::create_character(&client, account_a, "shared-hash", "PlayerA")
        .await
        .expect("first link should succeed");

    let result = db::create_character(&client, account_b, "shared-hash", "PlayerB").await;
    assert!(
        result.is_err(),
        "the same account_hash should not be linkable to two accounts"
    );

    // Confirms the endpoint handler's own conflict check has something real to compare
    // against: the existing row still belongs to account_a, not account_b.
    let found = db::find_character_by_account_hash(&client, "shared-hash")
        .await
        .expect("query failed")
        .expect("character should exist");
    assert_eq!(found.account_id, account_a);
}

#[tokio::test]
async fn test_character_cap_per_account() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "capped@example.com", &password_hash)
        .await
        .unwrap();

    for i in 0..db::CHARACTER_CAP_PER_ACCOUNT {
        db::create_character(&client, account_id, &format!("hash-cap-{}", i), "Alt")
            .await
            .expect("should be able to create character under the cap");
    }

    let count = db::count_characters_for_account(&client, account_id)
        .await
        .expect("query failed");
    assert_eq!(count, db::CHARACTER_CAP_PER_ACCOUNT);
}

#[tokio::test]
async fn test_discord_account_can_log_in_via_session_after_linking() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let account_id = db::create_account_with_discord_id(&client, "777888999")
        .await
        .unwrap();

    let token = crypto::new_session_token();
    let token_hash = crypto::session_token_hash(&token);
    let expires_at = Utc::now() + Duration::days(30);
    db::create_account_session(&client, account_id, &token_hash, &expires_at)
        .await
        .expect("failed to create session");

    let account = db::get_account_by_session_token_hash(&client, &token_hash)
        .await
        .expect("query failed")
        .expect("session should resolve to the discord account");
    assert_eq!(account.id, account_id);
    assert_eq!(account.email, None);
}
