use crate::db;
use crate::error::ApiError;
use actix_web::{get, web, Error, HttpRequest, HttpResponse};
use chrono::Utc;
use deadpool_postgres::Pool;
use serde_json::json;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Instant;

static STARTED_AT: LazyLock<Instant> = LazyLock::new(Instant::now);

fn bearer_token(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.to_string())
}

// Health-ping endpoint polled by a Vantage instance to monitor this app.
// Runs unauthenticated if VANTAGE_API_KEY is unset — acceptable for dev only.
#[get("/vantage/ping")]
pub async fn vantage_ping(req: HttpRequest) -> HttpResponse {
    if let Ok(expected) = std::env::var("VANTAGE_API_KEY") {
        if !expected.is_empty() && bearer_token(&req).as_deref() != Some(expected.as_str()) {
            return HttpResponse::Unauthorized().finish();
        }
    }

    HttpResponse::Ok().json(json!({
        "ok": true,
        "name": "GroupScape",
        "version": std::env::var("APP_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
        "env": std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string()),
        "uptime": STARTED_AT.elapsed().as_secs(),
        "ts": Utc::now().to_rfc3339(),
        "commit": std::env::var("COMMIT_HASH").unwrap_or_else(|_| "unknown".to_string()),
        "commitDate": std::env::var("COMMIT_DATE").unwrap_or_else(|_| "unknown".to_string()),
        "commitMsg": std::env::var("COMMIT_MSG").unwrap_or_else(|_| "unknown".to_string()),
    }))
}

fn homepage_key_from_request(req: &HttpRequest) -> Option<String> {
    if let Some(header) = req.headers().get("X-Api-Key").and_then(|v| v.to_str().ok()) {
        return Some(header.to_string());
    }

    web::Query::<HashMap<String, String>>::from_query(req.query_string())
        .ok()
        .and_then(|q| q.get("apiKey").cloned())
}

// Flat customapi widget feed for gethomepage.dev. 503s until HOMEPAGE_API_KEY is set.
#[get("/homepage/stats")]
pub async fn homepage_stats(
    req: HttpRequest,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let expected = match std::env::var("HOMEPAGE_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            return Ok(
                HttpResponse::ServiceUnavailable().json(json!({ "error": "Not configured" }))
            )
        }
    };

    if homepage_key_from_request(&req).as_deref() != Some(expected.as_str()) {
        return Ok(HttpResponse::Unauthorized().finish());
    }

    let client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let stats = db::get_homepage_stats(&client).await?;

    Ok(HttpResponse::Ok().json(json!([
        { "label": "Active groups", "value": stats.active_groups },
        { "label": "Bound characters", "value": stats.bound_characters },
        { "label": "Online characters", "value": stats.online_characters },
    ])))
}
