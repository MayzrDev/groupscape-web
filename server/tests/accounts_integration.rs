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
    assert_eq!(account.email, "player@example.com");
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
    assert_eq!(account.email, "session@example.com");

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
