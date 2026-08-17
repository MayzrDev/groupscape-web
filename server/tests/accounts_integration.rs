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
async fn test_list_characters_for_account_orders_oldest_first() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "lister@example.com", &password_hash)
        .await
        .unwrap();

    let first = db::create_character(&client, account_id, "hash-first", "Zezima")
        .await
        .expect("failed to create character");
    let second = db::create_character(&client, account_id, "hash-second", "Woox")
        .await
        .expect("failed to create character");

    let characters = db::list_characters_for_account(&client, account_id)
        .await
        .expect("query failed");

    assert_eq!(characters.len(), 2);
    assert_eq!(characters[0].id, first.id);
    assert_eq!(characters[1].id, second.id);
}

#[tokio::test]
async fn test_list_characters_for_account_excludes_other_accounts() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_a = db::create_account(&client, "lister-a@example.com", &password_hash)
        .await
        .unwrap();
    let account_b = db::create_account(&client, "lister-b@example.com", &password_hash)
        .await
        .unwrap();
    db::create_character(&client, account_a, "hash-a", "Alt A")
        .await
        .expect("failed to create character");
    db::create_character(&client, account_b, "hash-b", "Alt B")
        .await
        .expect("failed to create character");

    let characters = db::list_characters_for_account(&client, account_a)
        .await
        .expect("query failed");

    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].account_hash, "hash-a");
}

#[tokio::test]
async fn test_delete_character_removes_it() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "unlinker@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-unlink-1", "Zezima")
        .await
        .expect("failed to create character");

    db::delete_character(&client, character.id)
        .await
        .expect("delete should succeed");

    let found = db::find_character_by_id(&client, character.id)
        .await
        .expect("query failed");
    assert!(found.is_none());
}

#[tokio::test]
async fn test_delete_character_cascades_group_link() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "unlinker-grouped@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-unlink-2", "Zezima")
        .await
        .expect("failed to create character");
    let group_id = create_test_group(&client, "unlinktest1").await;
    db::link_character_to_group(&mut client, character.id, account_id, group_id)
        .await
        .expect("linking should succeed");

    db::delete_character(&client, character.id)
        .await
        .expect("delete should succeed");

    let link = db::find_character_group_link(&client, character.id)
        .await
        .expect("query failed");
    assert!(link.is_none());
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

#[tokio::test]
async fn test_link_character_to_group_creates_link() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "grouper@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-group-1", "Zezima")
        .await
        .unwrap();
    let group_id = create_test_group(&client, "grouptest1").await;

    let link = db::link_character_to_group(&mut client, character.id, account_id, group_id)
        .await
        .expect("linking should succeed");
    assert_eq!(link.character_id, character.id);
    assert_eq!(link.group_id, group_id);

    let found = db::find_character_group_link(&client, character.id)
        .await
        .expect("query failed")
        .expect("link should exist");
    assert_eq!(found.group_id, group_id);

    let admin = db::get_group_admin_account_id(&client, group_id)
        .await
        .expect("query failed");
    assert_eq!(
        admin,
        Some(account_id),
        "the first account whose character links into a group becomes its admin"
    );
}

#[tokio::test]
async fn test_relinking_character_to_same_group_is_idempotent() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "regrouper@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-group-2", "Zezima")
        .await
        .unwrap();
    let group_id = create_test_group(&client, "grouptest2").await;

    db::link_character_to_group(&mut client, character.id, account_id, group_id)
        .await
        .expect("first link should succeed");
    let second = db::link_character_to_group(&mut client, character.id, account_id, group_id)
        .await
        .expect("re-linking to the same group should be idempotent, not an error");
    assert_eq!(second.group_id, group_id);
}

#[tokio::test]
async fn test_group_with_no_joins_has_no_admin() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let group_id = create_test_group(&client, "adminlesstest1").await;

    let admin = db::get_group_admin_account_id(&client, group_id)
        .await
        .expect("query failed");
    assert_eq!(admin, None);
}

#[tokio::test]
async fn test_second_account_joining_a_group_does_not_take_over_admin() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let first_account_id = db::create_account(&client, "firstjoiner@example.com", &password_hash)
        .await
        .unwrap();
    let second_account_id =
        db::create_account(&client, "secondjoiner@example.com", &password_hash)
            .await
            .unwrap();
    let first_character = db::create_character(&client, first_account_id, "hash-admin-1", "Zezima")
        .await
        .unwrap();
    let second_character =
        db::create_character(&client, second_account_id, "hash-admin-2", "Woox")
            .await
            .unwrap();
    let group_id = create_test_group(&client, "admintest1").await;

    db::link_character_to_group(&mut client, first_character.id, first_account_id, group_id)
        .await
        .expect("first join should succeed");
    db::link_character_to_group(&mut client, second_character.id, second_account_id, group_id)
        .await
        .expect("second join should succeed");

    let admin = db::get_group_admin_account_id(&client, group_id)
        .await
        .expect("query failed");
    assert_eq!(
        admin,
        Some(first_account_id),
        "admin stays the first account to join, not the most recent one"
    );
}

#[tokio::test]
async fn test_linking_character_to_different_group_is_conflict() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "doublegrouper@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-group-3", "Zezima")
        .await
        .unwrap();
    let first_group_id = create_test_group(&client, "grouptest3a").await;
    let second_group_id = create_test_group(&client, "grouptest3b").await;

    db::link_character_to_group(&mut client, character.id, account_id, first_group_id)
        .await
        .expect("first link should succeed");
    let result =
        db::link_character_to_group(&mut client, character.id, account_id, second_group_id)
            .await;
    assert!(matches!(result, Err(ApiError::CharacterAlreadyInGroupError)));

    let still_linked = db::find_character_group_link(&client, character.id)
        .await
        .expect("query failed")
        .expect("link should still exist");
    assert_eq!(still_linked.group_id, first_group_id, "conflicting link attempt must not move the character");
}

#[tokio::test]
async fn test_find_character_group_link_returns_none_when_unlinked() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "unlinked@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-group-4", "Zezima")
        .await
        .unwrap();

    let found = db::find_character_group_link(&client, character.id)
        .await
        .expect("query failed");
    assert!(found.is_none());
}

#[tokio::test]
async fn test_get_account_by_id() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "by-id@example.com", &password_hash)
        .await
        .unwrap();

    let account = db::get_account_by_id(&client, account_id)
        .await
        .expect("query failed")
        .expect("account should exist");
    assert_eq!(account.id, account_id);
    assert_eq!(account.email.as_deref(), Some("by-id@example.com"));

    let missing = db::get_account_by_id(&client, account_id + 999)
        .await
        .expect("query failed");
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_update_account_email() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "old@example.com", &password_hash)
        .await
        .unwrap();

    db::update_account_email(&client, account_id, "new@example.com")
        .await
        .expect("failed to update email");

    let account = db::get_account_by_id(&client, account_id)
        .await
        .expect("query failed")
        .expect("account should exist");
    assert_eq!(account.email.as_deref(), Some("new@example.com"));
}

#[tokio::test]
async fn test_update_account_email_rejects_duplicate() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    db::create_account(&client, "taken@example.com", &password_hash)
        .await
        .unwrap();
    let account_id = db::create_account(&client, "mine@example.com", &password_hash)
        .await
        .unwrap();

    let result = db::update_account_email(&client, account_id, "taken@example.com").await;
    assert!(matches!(result, Err(ApiError::EmailAlreadyRegisteredError)));
}

#[tokio::test]
async fn test_update_account_password() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("original-password").unwrap();
    let account_id = db::create_account(&client, "changer@example.com", &password_hash)
        .await
        .unwrap();

    let new_hash = crypto::hash_password("brand-new-password").unwrap();
    db::update_account_password(&client, account_id, &new_hash)
        .await
        .expect("failed to update password");

    let account = db::get_account_by_id(&client, account_id)
        .await
        .expect("query failed")
        .expect("account should exist");
    assert!(crypto::verify_password(
        "brand-new-password",
        account.password_hash.as_deref().unwrap()
    ));
    assert!(!crypto::verify_password(
        "original-password",
        account.password_hash.as_deref().unwrap()
    ));
}

#[tokio::test]
async fn test_delete_account_cascades_characters_and_sessions() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "deleteme@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-delete-1", "Zezima")
        .await
        .unwrap();
    let token_hash = crypto::session_token_hash("delete-account-session-token");
    let expires_at = Utc::now() + Duration::days(1);
    db::create_account_session(&client, account_id, &token_hash, &expires_at)
        .await
        .unwrap();

    db::delete_account(&client, account_id)
        .await
        .expect("failed to delete account");

    let account = db::get_account_by_id(&client, account_id)
        .await
        .expect("query failed");
    assert!(account.is_none());

    let character = db::find_character_by_id(&client, character.id)
        .await
        .expect("query failed");
    assert!(character.is_none(), "character should be cascade-deleted with the account");

    let session = db::get_account_by_session_token_hash(&client, &token_hash)
        .await
        .expect("query failed");
    assert!(session.is_none(), "session should be cascade-deleted with the account");
}

async fn get_member_names(client: &deadpool_postgres::Client, group_id: i64) -> Vec<String> {
    let rows = client
        .query(
            "SELECT member_name FROM groupscape.members WHERE group_id=$1 AND member_name != '@SHARED' ORDER BY member_name",
            &[&group_id],
        )
        .await
        .expect("query failed");
    rows.into_iter().map(|row| row.get(0)).collect()
}

#[tokio::test]
async fn test_ensure_member_for_linked_character_creates_row_when_none_exists() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "rsngroup1").await;

    let name = db::ensure_member_for_linked_character(&client, group_id, "rsn-hash-1", "Zezima")
        .await
        .expect("should create a member row");
    assert_eq!(name, "Zezima");
    assert_eq!(get_member_names(&client, group_id).await, vec!["Zezima"]);
}

#[tokio::test]
async fn test_ensure_member_for_linked_character_claims_pre_existing_typed_row() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "rsngroup2").await;

    db::add_group_member(&client, group_id, "Zezima")
        .await
        .expect("failed to add typed-in member");

    let name = db::ensure_member_for_linked_character(&client, group_id, "rsn-hash-2", "Zezima")
        .await
        .expect("should claim the existing row");
    assert_eq!(name, "Zezima");
    assert_eq!(get_member_names(&client, group_id).await, vec!["Zezima"]);

    // Claiming is idempotent - calling again with the same hash finds it via the hash lookup,
    // not the (now-tagged) name-claim path, and still resolves to the same row.
    let name_again =
        db::ensure_member_for_linked_character(&client, group_id, "rsn-hash-2", "Zezima")
            .await
            .expect("should resolve the already-claimed row");
    assert_eq!(name_again, "Zezima");
    assert_eq!(get_member_names(&client, group_id).await, vec!["Zezima"]);
}

#[tokio::test]
async fn test_ensure_member_for_linked_character_renames_on_display_rsn_change() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "rsngroup3").await;

    db::ensure_member_for_linked_character(&client, group_id, "rsn-hash-3", "OldName")
        .await
        .expect("should create a member row");

    let renamed =
        db::ensure_member_for_linked_character(&client, group_id, "rsn-hash-3", "NewName")
            .await
            .expect("should rename in place");
    assert_eq!(renamed, "NewName");
    assert_eq!(get_member_names(&client, group_id).await, vec!["NewName"]);
}

#[tokio::test]
async fn test_ensure_member_for_linked_character_respects_group_cap() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "rsngroup4").await;

    for i in 0..5 {
        db::ensure_member_for_linked_character(
            &client,
            group_id,
            &format!("rsn-hash-cap-{i}"),
            &format!("Member{i}"),
        )
        .await
        .expect("should create a member row under the cap");
    }

    let result =
        db::ensure_member_for_linked_character(&client, group_id, "rsn-hash-cap-6", "Member6")
            .await;
    assert!(matches!(result, Err(ApiError::GroupFullError)));
}

#[tokio::test]
async fn test_ensure_member_for_linked_character_scoped_per_group() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_a = create_test_group(&client, "rsngroup5a").await;
    let group_b = create_test_group(&client, "rsngroup5b").await;

    db::ensure_member_for_linked_character(&client, group_a, "rsn-hash-5", "Zezima")
        .await
        .expect("should create in group a");
    db::ensure_member_for_linked_character(&client, group_b, "rsn-hash-5", "Zezima")
        .await
        .expect("same account_hash in a different group should not conflict");

    assert_eq!(get_member_names(&client, group_a).await, vec!["Zezima"]);
    assert_eq!(get_member_names(&client, group_b).await, vec!["Zezima"]);
}
