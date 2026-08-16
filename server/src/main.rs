use server::admin;
use server::admin_auth_middleware::{AdminAuthenticateMiddlewareFactory, AdminLoginRateLimiter};
use server::auth_middleware::AuthenticateMiddlewareFactory;
use server::authed;
use server::config::Config;
use server::db;
use server::models;
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
    let config = config;
    let pool = config.pg.create_pool(None, NoTls).unwrap();
    env_logger::init_from_env(
        env_logger::Env::new().default_filter_or(config.logger.level.to_string()),
    );

    let mut client = pool.get().await.unwrap();
    db::update_schema(&mut client).await.unwrap();

    unauthed::start_ge_updater();
    unauthed::start_skills_aggregator(pool.clone());

    let update_batcher_pool = config.pg.create_pool(None, NoTls).unwrap();
    let (tx, rx) = mpsc::channel::<models::GroupMember>(10000);
    tokio::spawn(async move {
        update_batcher::background_worker(update_batcher_pool, rx, None).await;
    });
    let auth_cache = std::sync::Arc::new(server::auth_middleware::AuthenticationCache::new());
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
        let authed_scope = web::scope("/api/group/{group_name}")
            .wrap(AuthenticateMiddlewareFactory::new(auth_cache.clone()))
            .service(authed::update_group_member)
            .service(authed::get_group_data)
            .service(authed::add_group_member)
            .service(authed::delete_group_member)
            .service(authed::rename_group_member)
            .service(authed::rename_group)
            .service(authed::reroll_group_token)
            .service(authed::delete_group)
            .service(authed::am_i_logged_in)
            .service(authed::am_i_in_group)
            .service(authed::get_skill_data)
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
            .service(unauthed_scope)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
