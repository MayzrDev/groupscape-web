use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

/// `ConnectionManager`'s own initial-connect retry/backoff has no overall deadline - against an
/// unreachable host it was observed to keep retrying for 8+ minutes instead of failing fast,
/// which would otherwise hang server startup. Bound it explicitly.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Per-command deadline so a degraded/unreachable Redis (after a successful initial connect)
/// can never meaningfully slow down the request it's caching for - a cache lookup that can't
/// complete quickly is worse than no cache at all.
const COMMAND_TIMEOUT: Duration = Duration::from_millis(200);

/// Thin wrapper around a Redis connection for short-TTL response caching on read-heavy,
/// group-scoped aggregate endpoints (leaderboard, metric-data, sessions, loot log summary).
/// Entirely optional: with no `REDIS_URL` configured, or if the initial connection fails,
/// every method is a silent no-op and callers fall straight through to Postgres exactly as
/// they did before this cache existed. A cache read/write failure or timeout must never fail
/// or stall the request it's supporting, so every fallible path here collapses to `None`/`()`
/// rather than an `Err`.
#[derive(Clone)]
pub struct RedisCache {
    conn: Option<redis::aio::ConnectionManager>,
}

impl RedisCache {
    pub async fn connect(redis_url: Option<String>) -> Self {
        let Some(redis_url) = redis_url.filter(|url| !url.is_empty()) else {
            return Self { conn: None };
        };
        let client = match redis::Client::open(redis_url) {
            Ok(client) => client,
            Err(err) => {
                log::warn!("Redis cache disabled: invalid REDIS_URL: {}", err);
                return Self { conn: None };
            }
        };
        match tokio::time::timeout(CONNECT_TIMEOUT, client.get_connection_manager()).await {
            Ok(Ok(conn)) => Self { conn: Some(conn) },
            Ok(Err(err)) => {
                log::warn!("Redis cache disabled: failed to connect: {}", err);
                Self { conn: None }
            }
            Err(_) => {
                log::warn!("Redis cache disabled: connect timed out after {:?}", CONNECT_TIMEOUT);
                Self { conn: None }
            }
        }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut conn = self.conn.clone()?;
        let raw: Option<String> = tokio::time::timeout(COMMAND_TIMEOUT, conn.get(key))
            .await
            .ok()?
            .ok()?;
        serde_json::from_str(&raw?).ok()
    }

    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) {
        let Some(mut conn) = self.conn.clone() else {
            return;
        };
        let Ok(raw) = serde_json::to_string(value) else {
            return;
        };
        let _ = tokio::time::timeout(COMMAND_TIMEOUT, conn.set_ex::<_, _, ()>(key, raw, ttl_secs)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_cache_get_returns_none() {
        let cache = RedisCache::connect(None).await;
        assert_eq!(cache.get_json::<serde_json::Value>("any-key").await, None);
    }

    #[tokio::test]
    async fn disabled_cache_set_is_a_noop() {
        let cache = RedisCache::connect(None).await;
        cache.set_json("any-key", &serde_json::json!({"a": 1}), 15).await;
    }

    #[tokio::test]
    async fn connect_with_unreachable_url_disables_cache_instead_of_erroring() {
        let cache = RedisCache::connect(Some("redis://127.0.0.1:1".to_string())).await;
        assert_eq!(cache.get_json::<serde_json::Value>("any-key").await, None);
    }
}
