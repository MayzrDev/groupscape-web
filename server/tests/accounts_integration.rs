use chrono::{Duration, Utc};
use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod};
use std::env;
use tokio_postgres::NoTls;

use server::config::Config;
use server::crypto;
use server::db;
use server::error::ApiError;
use server::permissions::{require_group_permission, ACCOUNT_AUTH_HEADER};

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
    assert_eq!(
        account.password_hash.as_deref(),
        Some(password_hash.as_str())
    );
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
    assert!(
        account.is_none(),
        "disabled account should not authenticate"
    );
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

    let refreshed = db::update_character_display_rsn(&client, character.id, "NewName", None, None)
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
async fn test_update_character_display_rsn_sets_and_preserves_summary_stats() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "stats-refresher@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-stats", "Zezima")
        .await
        .unwrap();
    assert_eq!(character.combat_level, None);
    assert_eq!(character.total_level, None);

    let with_stats = db::update_character_display_rsn(
        &client,
        character.id,
        "Zezima",
        Some(126),
        Some(2277),
    )
    .await
    .expect("failed to set summary stats");
    assert_eq!(with_stats.combat_level, Some(126));
    assert_eq!(with_stats.total_level, Some(2277));

    // A later report that only sends RSN (older plugin build) shouldn't wipe out the
    // stats a newer build already reported.
    let rsn_only_update = db::update_character_display_rsn(&client, character.id, "Zezima", None, None)
        .await
        .expect("failed to refresh rsn");
    assert_eq!(rsn_only_update.combat_level, Some(126));
    assert_eq!(rsn_only_update.total_level, Some(2277));
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

    let characters = db::list_characters_for_account_with_group_status(&client, account_id)
        .await
        .expect("query failed");

    assert_eq!(characters.len(), 2);
    assert_eq!(characters[0].character.id, first.id);
    assert_eq!(characters[1].character.id, second.id);
    assert_eq!(characters[0].group_id, None);
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

    let characters = db::list_characters_for_account_with_group_status(&client, account_a)
        .await
        .expect("query failed");

    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].character.account_hash, "hash-a");
}

#[tokio::test]
async fn test_list_characters_for_account_reports_group_status() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "grouped-lister@example.com", &password_hash)
        .await
        .unwrap();
    let grouped = db::create_character(&client, account_id, "hash-grouped", "Grouped")
        .await
        .expect("failed to create character");
    let ungrouped = db::create_character(&client, account_id, "hash-ungrouped", "Ungrouped")
        .await
        .expect("failed to create character");
    let group_id = create_test_group(&client, "listerstatusgroup").await;
    db::link_character_to_group(&mut client, grouped.id, account_id, group_id)
        .await
        .expect("failed to link character to group");

    let characters = db::list_characters_for_account_with_group_status(&client, account_id)
        .await
        .expect("query failed");

    assert_eq!(characters.len(), 2);
    let grouped_entry = characters.iter().find(|c| c.character.id == grouped.id).unwrap();
    let ungrouped_entry = characters.iter().find(|c| c.character.id == ungrouped.id).unwrap();
    assert_eq!(grouped_entry.group_id, Some(group_id));
    assert_eq!(ungrouped_entry.group_id, None);
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
async fn test_unlink_character_from_group_allows_rejoin_to_a_different_group() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "leaver@example.com", &password_hash)
        .await
        .unwrap();
    let character = db::create_character(&client, account_id, "hash-leaver", "Zezima")
        .await
        .unwrap();
    let first_group_id = create_test_group(&client, "leavergroupone").await;
    let second_group_id = create_test_group(&client, "leavergrouptwo").await;

    db::link_character_to_group(&mut client, character.id, account_id, first_group_id)
        .await
        .expect("first link should succeed");

    // Switching straight to a different group is rejected while the old link still exists.
    let blocked =
        db::link_character_to_group(&mut client, character.id, account_id, second_group_id).await;
    assert!(blocked.is_err(), "should not be able to switch groups without leaving first");

    db::unlink_character_from_group(&client, character.id)
        .await
        .expect("leaving the group should succeed");
    assert!(
        db::find_character_group_link(&client, character.id)
            .await
            .expect("query failed")
            .is_none(),
        "character should have no group link after leaving"
    );

    let rejoined =
        db::link_character_to_group(&mut client, character.id, account_id, second_group_id)
            .await
            .expect("should be able to join a different group after leaving the first");
    assert_eq!(rejoined.group_id, second_group_id);
}

#[tokio::test]
async fn test_find_group_id_for_account_character_scopes_by_account_and_confirmed_status() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let owner_id = db::create_account(&client, "resolver-owner@example.com", &password_hash)
        .await
        .unwrap();
    let other_id = db::create_account(&client, "resolver-other@example.com", &password_hash)
        .await
        .unwrap();
    let group_id = create_test_group(&client, "resolvergroup").await;

    let confirmed = db::create_character(&client, owner_id, "hash-resolver-confirmed", "Confirmed")
        .await
        .unwrap();
    db::link_character_to_group(&mut client, confirmed.id, owner_id, group_id)
        .await
        .expect("linking should succeed");

    assert_eq!(
        db::find_group_id_for_account_character(&client, owner_id, "resolvergroup")
            .await
            .expect("query failed"),
        Some(group_id)
    );

    // A different account's characters don't resolve, even for the same group name.
    assert_eq!(
        db::find_group_id_for_account_character(&client, other_id, "resolvergroup")
            .await
            .expect("query failed"),
        None
    );

    // A pending (unconfirmed) character's group membership doesn't count for this account
    // either, even though it can still be linked at the DB layer.
    let pending_group_id = create_test_group(&client, "resolvergrouppending").await;
    let pending = db::create_pending_character(&client, other_id, "hash-resolver-pending")
        .await
        .unwrap();
    db::link_character_to_group(&mut client, pending.id, other_id, pending_group_id)
        .await
        .expect("linking a pending character should still succeed at the DB layer");
    assert_eq!(
        db::find_group_id_for_account_character(&client, other_id, "resolvergrouppending")
            .await
            .expect("query failed"),
        None,
        "an unconfirmed character's group membership shouldn't grant account-session access"
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
    let second_account_id = db::create_account(&client, "secondjoiner@example.com", &password_hash)
        .await
        .unwrap();
    let first_character = db::create_character(&client, first_account_id, "hash-admin-1", "Zezima")
        .await
        .unwrap();
    let second_character = db::create_character(&client, second_account_id, "hash-admin-2", "Woox")
        .await
        .unwrap();
    let group_id = create_test_group(&client, "admintest1").await;

    db::link_character_to_group(&mut client, first_character.id, first_account_id, group_id)
        .await
        .expect("first join should succeed");
    db::link_character_to_group(
        &mut client,
        second_character.id,
        second_account_id,
        group_id,
    )
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
        db::link_character_to_group(&mut client, character.id, account_id, second_group_id).await;
    assert!(matches!(
        result,
        Err(ApiError::CharacterAlreadyInGroupError)
    ));

    let still_linked = db::find_character_group_link(&client, character.id)
        .await
        .expect("query failed")
        .expect("link should still exist");
    assert_eq!(
        still_linked.group_id, first_group_id,
        "conflicting link attempt must not move the character"
    );
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
    assert!(
        character.is_none(),
        "character should be cascade-deleted with the account"
    );

    let session = db::get_account_by_session_token_hash(&client, &token_hash)
        .await
        .expect("query failed");
    assert!(
        session.is_none(),
        "session should be cascade-deleted with the account"
    );
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

async fn create_linked_account(
    client: &mut deadpool_postgres::Client,
    email: &str,
    account_hash: &str,
    rsn: &str,
    group_id: i64,
) -> i64 {
    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(client, email, &password_hash)
        .await
        .unwrap();
    let character = db::create_character(client, account_id, account_hash, rsn)
        .await
        .unwrap();
    db::link_character_to_group(client, character.id, account_id, group_id)
        .await
        .expect("linking should succeed");
    account_id
}

#[tokio::test]
async fn test_linking_a_character_creates_default_all_false_permissions() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest1").await;

    let account_id = create_linked_account(
        &mut client,
        "perm1@example.com",
        "hash-perm-1",
        "Zezima",
        group_id,
    )
    .await;

    let permissions = db::get_group_permissions(&client, group_id, account_id)
        .await
        .expect("query failed")
        .expect("permissions row should exist after linking");
    assert_eq!(
        permissions.flags,
        server::models::PermissionFlags::default()
    );
}

#[tokio::test]
async fn test_update_group_permissions_patches_only_provided_fields() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest2").await;

    // First linked account becomes admin (whose row this test must not touch, see
    // `test_update_group_permissions_rejects_writes_to_the_admin`); the account under test
    // links second so it stays a plain member with real toggle-able flags.
    create_linked_account(
        &mut client,
        "perm2admin@example.com",
        "hash-perm-2admin",
        "Zezima",
        group_id,
    )
    .await;
    let account_id = create_linked_account(
        &mut client,
        "perm2@example.com",
        "hash-perm-2",
        "Woox",
        group_id,
    )
    .await;

    let patch = server::models::PermissionFlagsPatch {
        kick_members: Some(true),
        manage_settings: Some(true),
        ..Default::default()
    };
    let updated = db::update_group_permissions(&client, group_id, account_id, patch)
        .await
        .expect("query failed")
        .expect("permissions row should exist");
    assert!(updated.flags.kick_members);
    assert!(updated.flags.manage_settings);
    assert!(
        !updated.flags.invite_members,
        "untouched fields should stay false"
    );

    let second_patch = server::models::PermissionFlagsPatch {
        kick_members: Some(false),
        ..Default::default()
    };
    let second_update = db::update_group_permissions(&client, group_id, account_id, second_patch)
        .await
        .expect("query failed")
        .expect("permissions row should exist");
    assert!(!second_update.flags.kick_members);
    assert!(
        second_update.flags.manage_settings,
        "a later partial patch must not clobber fields it didn't mention"
    );
}

#[tokio::test]
async fn test_update_group_permissions_returns_none_for_non_member() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest3").await;

    let password_hash = crypto::hash_password("hunter22").unwrap();
    let account_id = db::create_account(&client, "perm3@example.com", &password_hash)
        .await
        .unwrap();

    let patch = server::models::PermissionFlagsPatch {
        kick_members: Some(true),
        ..Default::default()
    };
    let result = db::update_group_permissions(&client, group_id, account_id, patch)
        .await
        .expect("query failed");
    assert!(
        result.is_none(),
        "an account with no membership row has nothing to patch"
    );
}

#[tokio::test]
async fn test_list_group_permissions_returns_every_member_in_the_group() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest4").await;
    let other_group_id = create_test_group(&client, "permtest4-other").await;

    let account_a = create_linked_account(
        &mut client,
        "perm4a@example.com",
        "hash-perm-4a",
        "Zezima",
        group_id,
    )
    .await;
    let account_b = create_linked_account(
        &mut client,
        "perm4b@example.com",
        "hash-perm-4b",
        "Woox",
        group_id,
    )
    .await;
    create_linked_account(
        &mut client,
        "perm4c@example.com",
        "hash-perm-4c",
        "Elsewhere",
        other_group_id,
    )
    .await;

    let permissions = db::list_group_permissions(&client, group_id)
        .await
        .expect("query failed");
    let mut account_ids: Vec<i64> = permissions.iter().map(|p| p.account_id).collect();
    account_ids.sort();
    let mut expected = vec![account_a, account_b];
    expected.sort();
    assert_eq!(account_ids, expected);
}

#[tokio::test]
async fn test_list_group_member_permissions_includes_display_rsn_and_admin_flag() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest5").await;

    // First linked account becomes the group admin (see "set group admin = first user to
    // create group").
    let admin_account = create_linked_account(
        &mut client,
        "perm5admin@example.com",
        "hash-perm-5admin",
        "Zezima",
        group_id,
    )
    .await;
    let member_account = create_linked_account(
        &mut client,
        "perm5member@example.com",
        "hash-perm-5member",
        "Woox",
        group_id,
    )
    .await;

    let permissions = db::list_group_member_permissions(&client, group_id)
        .await
        .expect("query failed");
    assert_eq!(permissions.len(), 2);

    let admin_row = permissions
        .iter()
        .find(|p| p.account_id == admin_account)
        .expect("admin row missing");
    assert_eq!(admin_row.display_rsn, "Zezima");
    assert!(admin_row.is_admin);

    let member_row = permissions
        .iter()
        .find(|p| p.account_id == member_account)
        .expect("member row missing");
    assert_eq!(member_row.display_rsn, "Woox");
    assert!(!member_row.is_admin);
    assert!(!member_row.flags.kick_members);
}

#[tokio::test]
async fn test_get_effective_permission_flags_admin_returns_all_true() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest4a").await;

    let admin_account_id = create_linked_account(
        &mut client,
        "perm4aadmin@example.com",
        "hash-perm-4aa",
        "Zezima",
        group_id,
    )
    .await;

    let flags = db::get_effective_permission_flags(&client, group_id, admin_account_id)
        .await
        .expect("query failed");
    assert_eq!(
        flags,
        server::models::PermissionFlags::all_true(),
        "the admin's effective flags must read as every permission on, even though the stored row is still all-false"
    );
}

#[tokio::test]
async fn test_get_effective_permission_flags_non_admin_returns_stored_flags() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest4b").await;

    create_linked_account(
        &mut client,
        "perm4badmin@example.com",
        "hash-perm-4ba",
        "Zezima",
        group_id,
    )
    .await;
    let member_account_id = create_linked_account(
        &mut client,
        "perm4bmember@example.com",
        "hash-perm-4bb",
        "Woox",
        group_id,
    )
    .await;

    let before = db::get_effective_permission_flags(&client, group_id, member_account_id)
        .await
        .expect("query failed");
    assert_eq!(
        before,
        server::models::PermissionFlags::default(),
        "a non-admin member's effective flags default to every permission off"
    );

    let patch = server::models::PermissionFlagsPatch {
        kick_members: Some(true),
        ..Default::default()
    };
    db::update_group_permissions(&client, group_id, member_account_id, patch)
        .await
        .expect("query failed");

    let after = db::get_effective_permission_flags(&client, group_id, member_account_id)
        .await
        .expect("query failed");
    assert!(
        after.kick_members,
        "the granted flag should be reflected in the effective flags"
    );
}

#[tokio::test]
async fn test_has_group_permission_admin_is_always_true() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest5").await;

    let admin_account_id = create_linked_account(
        &mut client,
        "perm5admin@example.com",
        "hash-perm-5a",
        "Zezima",
        group_id,
    )
    .await;

    let has_permission = db::has_group_permission(
        &client,
        group_id,
        admin_account_id,
        server::models::PermissionKey::ManagePermissions,
    )
    .await
    .expect("query failed");
    assert!(
        has_permission,
        "the group admin has every permission implicitly, regardless of stored flags"
    );
}

#[tokio::test]
async fn test_has_group_permission_non_admin_follows_stored_flags() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest6").await;

    // First linked account becomes admin.
    create_linked_account(
        &mut client,
        "perm6admin@example.com",
        "hash-perm-6a",
        "Zezima",
        group_id,
    )
    .await;
    let member_account_id = create_linked_account(
        &mut client,
        "perm6member@example.com",
        "hash-perm-6b",
        "Woox",
        group_id,
    )
    .await;

    let before = db::has_group_permission(
        &client,
        group_id,
        member_account_id,
        server::models::PermissionKey::KickMembers,
    )
    .await
    .expect("query failed");
    assert!(!before, "a non-admin member defaults to every flag off");

    let patch = server::models::PermissionFlagsPatch {
        kick_members: Some(true),
        ..Default::default()
    };
    db::update_group_permissions(&client, group_id, member_account_id, patch)
        .await
        .expect("query failed");

    let after = db::has_group_permission(
        &client,
        group_id,
        member_account_id,
        server::models::PermissionKey::KickMembers,
    )
    .await
    .expect("query failed");
    assert!(after, "the flag should now read true after being granted");
}

#[tokio::test]
async fn test_update_group_permissions_rejects_writes_to_the_admin() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest7").await;

    let admin_account_id = create_linked_account(
        &mut client,
        "perm7admin@example.com",
        "hash-perm-7a",
        "Zezima",
        group_id,
    )
    .await;

    let patch = server::models::PermissionFlagsPatch {
        kick_members: Some(false),
        ..Default::default()
    };
    let result = db::update_group_permissions(&client, group_id, admin_account_id, patch).await;
    assert!(matches!(
        result,
        Err(ApiError::CannotModifyGroupAdminPermissionsError)
    ));

    let has_permission = db::has_group_permission(
        &client,
        group_id,
        admin_account_id,
        server::models::PermissionKey::KickMembers,
    )
    .await
    .expect("query failed");
    assert!(
        has_permission,
        "the rejected write must not have demoted the admin's implicit access"
    );
}

#[tokio::test]
async fn test_require_group_permission_rejects_missing_account_header() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest7b").await;

    let req = actix_web::test::TestRequest::default().to_http_request();
    let result = require_group_permission(
        &req,
        &client,
        group_id,
        server::models::PermissionKey::ManageSettings,
    )
    .await;
    assert!(matches!(result, Err(ApiError::AccountAuthRequiredError)));
}

#[tokio::test]
async fn test_require_group_permission_rejects_permission_denied() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest8").await;

    // First linked account becomes admin; second is a plain member with no flags granted.
    create_linked_account(
        &mut client,
        "perm8admin@example.com",
        "hash-perm-8a",
        "Zezima",
        group_id,
    )
    .await;
    let member_account_id = create_linked_account(
        &mut client,
        "perm8member@example.com",
        "hash-perm-8b",
        "Woox",
        group_id,
    )
    .await;

    let token = "require-permission-test-token";
    let token_hash = crypto::session_token_hash(token);
    let expires_at = Utc::now() + Duration::days(30);
    db::create_account_session(&client, member_account_id, &token_hash, &expires_at)
        .await
        .unwrap();

    let req = actix_web::test::TestRequest::default()
        .insert_header((ACCOUNT_AUTH_HEADER, token))
        .to_http_request();
    let result = require_group_permission(
        &req,
        &client,
        group_id,
        server::models::PermissionKey::ManageSettings,
    )
    .await;
    assert!(matches!(result, Err(ApiError::PermissionDeniedError)));
}

#[tokio::test]
async fn test_require_group_permission_allows_admin() {
    let _guard = TEST_MUTEX.lock().await;
    let pool = create_test_pool().await;
    setup(&pool).await;
    let mut client = pool.get().await.unwrap();
    let group_id = create_test_group(&client, "permtest9").await;

    let admin_account_id = create_linked_account(
        &mut client,
        "perm9admin@example.com",
        "hash-perm-9a",
        "Zezima",
        group_id,
    )
    .await;

    let token = "require-permission-admin-token";
    let token_hash = crypto::session_token_hash(token);
    let expires_at = Utc::now() + Duration::days(30);
    db::create_account_session(&client, admin_account_id, &token_hash, &expires_at)
        .await
        .unwrap();

    let req = actix_web::test::TestRequest::default()
        .insert_header((ACCOUNT_AUTH_HEADER, token))
        .to_http_request();
    let account_id = require_group_permission(
        &req,
        &client,
        group_id,
        server::models::PermissionKey::ManageSettings,
    )
    .await
    .expect("admin should always have permission");
    assert_eq!(account_id, admin_account_id);
}
