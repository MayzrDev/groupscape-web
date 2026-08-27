use server::account_auth_middleware::AccountAuthenticateMiddlewareFactory;
use server::accounts;
use server::admin;
use server::admin_auth_middleware::{AdminAuthenticateMiddlewareFactory, AdminLoginRateLimiter};
use server::auth_middleware::AuthenticateMiddlewareFactory;
use server::authed;
use server::character_auth_middleware::CharacterAuthenticateMiddlewareFactory;
use server::config::Config;
use server::db;
use server::models;
use server::push;
use server::unauthed;
use server::update_batcher;
use server::vantage;
use server::websocket;

use actix_cors::Cors;
use actix_web::{http::header, middleware, web, App, HttpServer};
use tokio::sync::mpsc;
use tokio_postgres::NoTls;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mut config = Config::from_env().unwrap();
    // The admin token is supplied raw via env var (never written to config.toml)
    // and hashed once at startup, mirroring how group tokens are only ever
    // stored/compared as hashes.
    if let Ok(admin_token) = std::env::var("ADMIN_TOKEN") {
        if !admin_token.is_empty() {
            config.admin.enabled = true;
            config.admin.token_hash = server::crypto::token_hash(&admin_token, "admin");
        }
    }
    // Discord OAuth login is optional - only enabled once all three are supplied, mirroring
    // the admin-token pattern above. None of these ever live in config.toml.
    if let (Ok(client_id), Ok(client_secret), Ok(redirect_uri)) = (
        std::env::var("DISCORD_CLIENT_ID"),
        std::env::var("DISCORD_CLIENT_SECRET"),
        std::env::var("DISCORD_REDIRECT_URI"),
    ) {
        if !client_id.is_empty() && !client_secret.is_empty() && !redirect_uri.is_empty() {
            config.discord.enabled = true;
            config.discord.client_id = client_id;
            config.discord.client_secret = client_secret;
            config.discord.redirect_uri = redirect_uri;
        }
    }
    // Web push is optional - only enabled once all three VAPID vars are supplied, mirroring
    // the Discord-OAuth pattern above. None of these ever live in config.toml.
    if let (Ok(vapid_public_key), Ok(vapid_private_key), Ok(vapid_subject)) = (
        std::env::var("VAPID_PUBLIC_KEY"),
        std::env::var("VAPID_PRIVATE_KEY"),
        std::env::var("VAPID_SUBJECT"),
    ) {
        if !vapid_public_key.is_empty() && !vapid_private_key.is_empty() && !vapid_subject.is_empty() {
            config.push.enabled = true;
            config.push.vapid_public_key = vapid_public_key;
            config.push.vapid_private_key = vapid_private_key;
            config.push.vapid_subject = vapid_subject;
        }
    }
    config.web_origin = std::env::var("WEB_ORIGIN").unwrap_or_default();
    let config = config;
    let pool = config.pg.create_pool(None, NoTls).unwrap();
    env_logger::init_from_env(
        env_logger::Env::new().default_filter_or(config.logger.level.to_string()),
    );

    // Postgres reporting healthy (pg_isready) doesn't guarantee it's ready to serve this
    // process's first connection/migration right away, especially on a fresh `docker compose up`
    // recreate. Retry instead of unwrap()-panicking the whole process on a transient race.
    let mut client = {
        let mut attempt = 0;
        loop {
            match pool.get().await {
                Ok(client) => break client,
                Err(err) if attempt < 9 => {
                    attempt += 1;
                    log::warn!(
                        "Failed to get DB connection on startup (attempt {}/10): {}",
                        attempt,
                        err
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                }
                Err(err) => panic!("Failed to get DB connection on startup after 10 attempts: {}", err),
            }
        }
    };
    {
        let mut attempt = 0;
        loop {
            match db::update_schema(&mut client).await {
                Ok(()) => break,
                Err(err) if attempt < 9 => {
                    attempt += 1;
                    log::warn!(
                        "Failed to run schema migrations on startup (attempt {}/10): {}",
                        attempt,
                        err
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                }
                Err(err) => panic!("Failed to run schema migrations on startup after 10 attempts: {}", err),
            }
        }
    }

    // Opt-in only: a fresh local/dev DB has no demo group until either this runs once or
    // someone runs `cargo run --bin seed` manually. Production never sets this - the demo group
    // there is kept fresh by the dedicated reset sidecar (see docker-compose.prod.yml) instead.
    if std::env::var("AUTO_SEED_DEMO_DATA").is_ok_and(|value| value == "true") {
        if let Err(err) = server::demo_seed::run(&mut client, false).await {
            log::warn!("AUTO_SEED_DEMO_DATA seed failed: {}", err);
        }
    }
    {
        let demo_group_id = db::get_group_id_by_name(&client, server::demo::DEMO_GROUP_NAME)
            .await
            .unwrap_or(None);
        server::demo::init(demo_group_id);
    }

    unauthed::start_ge_updater();
    unauthed::start_skills_aggregator(pool.clone());
    unauthed::start_session_idle_closer(pool.clone());
    unauthed::start_bank_value_snapshotter(pool.clone());
    unauthed::start_bank_value_aggregator(pool.clone());

    let update_batcher_pool = config.pg.create_pool(None, NoTls).unwrap();
    let (tx, rx) = mpsc::channel::<models::GroupMember>(10000);
    tokio::spawn(async move {
        update_batcher::background_worker(update_batcher_pool, rx, None).await;
    });
    let auth_cache = std::sync::Arc::new(server::auth_middleware::AuthenticationCache::new());
    let account_auth_cache =
        std::sync::Arc::new(server::account_auth_middleware::AccountAuthenticationCache::new());
    let character_auth_cache = std::sync::Arc::new(
        server::character_auth_middleware::CharacterAuthenticationCache::new(),
    );
    let admin_rate_limiter = std::sync::Arc::new(AdminLoginRateLimiter::new());
    let broadcast_registry = web::Data::new(websocket::GroupBroadcastRegistry::new());
    let config_data = web::Data::new(config.clone());

    HttpServer::new(move || {
        let unauthed_scope = web::scope("/api")
            .service(unauthed::create_group)
            .service(unauthed::get_ge_prices)
            .service(unauthed::captcha_enabled)
            .service(vantage::vantage_ping)
            .service(vantage::homepage_stats);
        // Both the public (register/login) and bearer-token-gated account routes live under
        // this single `/api/account` scope, with the gated ones nested in an inner `""`-prefix
        // scope carrying the auth middleware - two *separate* top-level scopes sharing the same
        // literal prefix string ("/api/account" registered twice via `App::service`) silently
        // shadow each other in actix-web, so only the first-registered one's routes were ever
        // reachable. That's what made every authed account endpoint (`/me`, `/characters`, ...)
        // 404 in production regardless of token validity.
        let account_scope = web::scope("/api/account")
            .app_data(web::Data::from(account_auth_cache.clone()))
            .service(accounts::register)
            .service(accounts::login)
            .service(accounts::discord_redirect)
            .service(accounts::discord_callback)
            .service(push::vapid_public_key)
            .service(
                web::scope("")
                    .wrap(AccountAuthenticateMiddlewareFactory::new(
                        account_auth_cache.clone(),
                    ))
                    .app_data(web::Data::from(character_auth_cache.clone()))
                    .service(accounts::me)
                    .service(accounts::update_username)
                    .service(accounts::change_password)
                    .service(accounts::delete_account)
                    .service(accounts::list_characters)
                    .service(accounts::link_character)
                    .service(accounts::unlink_character)
                    .service(accounts::link_character_to_group)
                    .service(accounts::leave_group)
                    .service(accounts::regenerate_api_key)
                    .service(accounts::discord_link_redirect)
                    .service(accounts::confirm_character)
                    .service(accounts::remove_pending_character)
                    .service(accounts::get_character_portrait)
                    .service(push::subscribe)
                    .service(push::unsubscribe),
            );
        // The site's own `/group` dashboard (a browser viewing a group's live data via the
        // group's invite-code token) - unrelated to the plugin, unchanged by the account-API-key
        // redesign. Carries the exact same handler list as before.
        let group_dashboard_scope = web::scope("/api/group/{group_name}")
            .wrap(AuthenticateMiddlewareFactory::new(auth_cache.clone()))
            // These 10 are also mounted (with different auth) under `character_scope` below,
            // via `web::resource(...).to(...)` rather than `#[route(...)]` + `.service(...)` -
            // see the comment there for why.
            .service(web::resource("/update-group-member").route(web::post().to(authed::update_group_member)))
            .service(web::resource("/get-activity-events").route(web::get().to(authed::get_activity_events)))
            .service(web::resource("/get-sessions").route(web::get().to(authed::get_sessions)))
            .service(web::resource("/get-loot-bosses").route(web::get().to(authed::get_loot_bosses)))
            .service(web::resource("/get-loot-summary").route(web::get().to(authed::get_loot_summary)))
            .service(web::resource("/get-loot-split").route(web::get().to(authed::get_loot_split)))
            .service(web::resource("/am-i-logged-in").route(web::get().to(authed::am_i_logged_in)))
            .service(web::resource("/am-i-in-group").route(web::get().to(authed::am_i_in_group)))
            .service(web::resource("/get-skill-data").route(web::get().to(authed::get_skill_data)))
            .service(web::resource("/get-metric-data").route(web::get().to(authed::get_metric_data)))
            .service(web::resource("/get-leaderboard").route(web::get().to(authed::get_leaderboard)))
            .service(web::resource("/collection-log").route(web::get().to(authed::get_collection_log)))
            .service(web::resource("/get-item-bonuses").route(web::get().to(authed::get_item_bonuses)))
            .service(authed::get_group_data)
            .service(authed::delete_group_member)
            .service(authed::block_group_member)
            .service(authed::unblock_group_member)
            .service(authed::can_kick_members)
            .service(authed::get_blocked_members)
            .service(authed::get_group_permissions)
            .service(authed::get_my_permissions)
            .service(authed::update_group_permissions)
            .service(authed::update_member_color)
            .service(authed::get_discord_settings)
            .service(authed::update_discord_settings)
            .service(authed::rename_group)
            .service(authed::reroll_group_token)
            .service(authed::delete_group)
            .service(authed::get_portrait)
            .service(
                web::resource("/update-portrait/{member_name}")
                    .app_data(web::PayloadConfig::new(5_000_000))
                    .route(web::post().to(authed::update_portrait)),
            );
        // Plugin-facing routes needing a linked group - same handler functions as the group
        // dashboard scope above (they only ever read `Authenticated{group_id}`), just reached
        // via `{account_hash}` + an API key instead of `{group_name}` + a group token.
        // Both branches share the literal prefix `/api/characters/{account_hash}` and need
        // different `CharacterAuthenticateMiddlewareFactory` configs (group required or not).
        // Nesting two `web::scope("")` children under one outer scope (matching the account
        // scope's fix above) turned out *not* to isolate their `.wrap()`s from each other here -
        // empirically, a request for `/identify` (only ever registered in the `false` child)
        // still hit the `true` child's "must have a group" check and got rejected before
        // reaching its own handler. So instead every route gets its own `.wrap()`, on an
        // unwrapped outer scope - verbose, but each resource's middleware is unambiguous.
        let grouped_character_middleware =
            || CharacterAuthenticateMiddlewareFactory::new(character_auth_cache.clone(), true);
        // Plugin-facing routes exempted from the "must have a group" gate: a pending, ungrouped
        // character still needs to be identifiable (RSN) and get its portrait uploaded so the
        // site's confirm card has real data to show.
        let ungrouped_character_middleware =
            || CharacterAuthenticateMiddlewareFactory::new(character_auth_cache.clone(), false);
        let character_scope = web::scope("/api/characters/{account_hash}")
            .service(
                web::resource("/update-group-member")
                    .wrap(grouped_character_middleware())
                    .route(web::post().to(authed::update_group_member)),
            )
            .service(
                web::resource("/get-activity-events")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_activity_events)),
            )
            .service(
                web::resource("/get-sessions")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_sessions)),
            )
            .service(
                web::resource("/get-loot-summary")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_loot_summary)),
            )
            .service(
                web::resource("/get-loot-split")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_loot_split)),
            )
            .service(
                web::resource("/am-i-logged-in")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::am_i_logged_in)),
            )
            .service(
                web::resource("/am-i-in-group")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::am_i_in_group_for_character)),
            )
            .service(
                web::resource("/get-skill-data")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_skill_data)),
            )
            .service(
                web::resource("/get-metric-data")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_metric_data)),
            )
            .service(
                web::resource("/get-leaderboard")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_leaderboard)),
            )
            .service(
                web::resource("/collection-log")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_collection_log)),
            )
            .service(
                web::resource("/get-item-bonuses")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(authed::get_item_bonuses)),
            )
            .service(
                web::resource("/ws")
                    .wrap(grouped_character_middleware())
                    .route(web::get().to(websocket::party_overlay_ws)),
            )
            .service(
                web::resource("/identify")
                    .wrap(ungrouped_character_middleware())
                    .route(web::post().to(authed::identify_character)),
            )
            .service(
                web::resource("/update-portrait/{member_name}")
                    .app_data(web::PayloadConfig::new(5_000_000))
                    .wrap(ungrouped_character_middleware())
                    .route(web::post().to(authed::update_character_portrait)),
            );
        let admin_scope = web::scope("/api/admin")
            .wrap(AdminAuthenticateMiddlewareFactory::new(
                config_data.clone(),
                admin_rate_limiter.clone(),
            ))
            .service(admin::am_i_logged_in)
            .service(admin::list_groups)
            .service(admin::get_group)
            .service(admin::suspend_group)
            .service(admin::ban_group)
            .service(admin::unban_group)
            .service(admin::delete_group)
            .service(admin::delete_character)
            .service(admin::unlink_character_from_group)
            .service(admin::list_audit_log)
            .service(admin::accounts_summary)
            .service(admin::list_accounts)
            .service(admin::get_account)
            .service(admin::reset_password)
            .service(admin::set_account_status)
            .service(admin::soft_delete_account)
            .service(admin::hard_delete_account)
            .service(admin::list_account_sessions)
            .service(admin::revoke_account_session)
            .service(admin::revoke_all_account_sessions)
            .service(admin::set_account_username)
            .service(admin::clear_account_lockout)
            .service(admin::search)
            .service(admin::dashboard);
        let json_config = web::JsonConfig::default().limit(100000);
        let cors = Cors::default()
            .allow_any_origin()
            .send_wildcard()
            .allowed_methods(vec!["GET", "POST", "DELETE", "PUT", "OPTIONS"])
            .allowed_headers(vec![
                header::AUTHORIZATION,
                header::ACCEPT,
                header::CONTENT_TYPE,
                header::CONTENT_LENGTH,
                header::HeaderName::from_static("x-account-authorization"),
            ])
            .max_age(3600);
        App::new()
            .wrap(middleware::Logger::new(
                "\"%r\" %s %b \"%{User-Agent}i\" %D",
            ))
            .wrap(middleware::Compress::default())
            .wrap(cors)
            .app_data(web::PayloadConfig::new(100000))
            .app_data(json_config)
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(tx.clone()))
            .app_data(broadcast_registry.clone())
            .service(group_dashboard_scope)
            .service(character_scope)
            .service(admin_scope)
            .service(account_scope)
            .service(unauthed_scope)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
