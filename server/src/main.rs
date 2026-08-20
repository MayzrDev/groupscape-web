use server::account_auth_middleware::AccountAuthenticateMiddlewareFactory;
use server::accounts;
use server::admin;
use server::admin_auth_middleware::{AdminAuthenticateMiddlewareFactory, AdminLoginRateLimiter};
use server::auth_middleware::AuthenticateMiddlewareFactory;
use server::authed;
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
        if !vapid_public_key.is_empty()
            && !vapid_private_key.is_empty()
            && !vapid_subject.is_empty()
        {
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

    let mut client = pool.get().await.unwrap();
    db::update_schema(&mut client).await.unwrap();

    unauthed::start_ge_updater();
    unauthed::start_skills_aggregator(pool.clone());
    unauthed::start_session_idle_closer(pool.clone());
    unauthed::start_bank_value_snapshotter(pool.clone());

    let update_batcher_pool = config.pg.create_pool(None, NoTls).unwrap();
    let (tx, rx) = mpsc::channel::<models::GroupMember>(10000);
    tokio::spawn(async move {
        update_batcher::background_worker(update_batcher_pool, rx, None).await;
    });
    let auth_cache = std::sync::Arc::new(server::auth_middleware::AuthenticationCache::new());
    let account_auth_cache =
        std::sync::Arc::new(server::account_auth_middleware::AccountAuthenticationCache::new());
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
        let account_scope = web::scope("/api/account")
            .service(accounts::register)
            .service(accounts::login)
            .service(accounts::discord_redirect)
            .service(accounts::discord_callback)
            .service(push::vapid_public_key);
        let authed_account_scope = web::scope("/api/account")
            .wrap(AccountAuthenticateMiddlewareFactory::new(
                account_auth_cache.clone(),
            ))
            .service(accounts::me)
            .service(accounts::update_email)
            .service(accounts::change_password)
            .service(accounts::delete_account)
            .service(accounts::list_characters)
            .service(accounts::link_character)
            .service(accounts::unlink_character)
            .service(accounts::link_character_to_group)
            .service(push::subscribe)
            .service(push::unsubscribe);
        let authed_scope = web::scope("/api/group/{group_name}")
            .wrap(AuthenticateMiddlewareFactory::new(auth_cache.clone()))
            .service(authed::update_group_member)
            .service(authed::get_group_data)
            .service(authed::get_activity_events)
            .service(authed::get_sessions)
            .service(authed::get_loot_summary)
            .service(authed::get_loot_split)
            .service(authed::delete_group_member)
            .service(authed::block_group_member)
            .service(authed::unblock_group_member)
            .service(authed::can_kick_members)
            .service(authed::get_blocked_members)
            .service(authed::get_group_permissions)
            .service(authed::get_my_permissions)
            .service(authed::update_group_permissions)
            .service(authed::get_discord_settings)
            .service(authed::update_discord_settings)
            .service(authed::get_group_goals)
            .service(authed::create_group_goal)
            .service(authed::update_group_goal)
            .service(authed::update_group_goal_status)
            .service(authed::delete_group_goal)
            .service(authed::rename_group)
            .service(authed::reroll_group_token)
            .service(authed::delete_group)
            .service(authed::am_i_logged_in)
            .service(authed::am_i_in_group)
            .service(authed::get_skill_data)
            .service(authed::get_leaderboard)
            .service(authed::get_collection_log)
            .service(authed::get_portrait)
            .service(
                web::resource("/update-portrait/{member_name}")
                    .app_data(web::PayloadConfig::new(5_000_000))
                    .route(web::post().to(authed::update_portrait)),
            )
            .service(websocket::party_overlay_ws);
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
            .service(admin::list_feature_flags)
            .service(admin::set_feature_flag)
            .service(admin::list_audit_log)
            .service(admin::accounts_summary);
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
            .service(authed_scope)
            .service(admin_scope)
            .service(account_scope)
            .service(authed_account_scope)
            .service(unauthed_scope)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
