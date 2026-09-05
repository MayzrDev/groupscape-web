use crate::db;
use crate::error::ApiError;
use crate::models::Account;
use actix_web::{
    body::BoxBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web, Error, FromRequest, HttpMessage, HttpRequest,
};
use deadpool_postgres::Pool;
use futures_util::{
    future::{ready, LocalBoxFuture, Ready},
    FutureExt,
};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

const ACCOUNT_AUTH_CACHE_TTL: Duration = Duration::from_secs(60);
const ACCOUNT_AUTH_CACHE_MAX_ENTRIES: usize = 10_000;
const ACCOUNT_AUTH_CACHE_SWEEP_INTERVAL: Duration = Duration::from_secs(60);

struct CachedAccount {
    account: Account,
    expires_at: Instant,
}

struct AccountAuthenticationCacheInner {
    entries: HashMap<String, CachedAccount>,
    last_sweep: Instant,
}

/// Caches session-token -> account lookups by token hash, same shape as the group
/// `AuthenticationCache` - avoids a DB round trip on every request from a logged-in account.
pub struct AccountAuthenticationCache {
    inner: RwLock<AccountAuthenticationCacheInner>,
}

impl AccountAuthenticationCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(AccountAuthenticationCacheInner {
                entries: HashMap::new(),
                last_sweep: Instant::now(),
            }),
        }
    }

    fn get(&self, token_hash: &str) -> Option<Account> {
        let inner = self.inner.read().ok()?;
        inner
            .entries
            .get(token_hash)
            .filter(|entry| entry.expires_at > Instant::now())
            .map(|entry| Account {
                id: entry.account.id,
                username: entry.account.username.clone(),
                discord_name: entry.account.discord_name.clone(),
                created_at: entry.account.created_at,
                must_change_password: entry.account.must_change_password,
            })
    }

    /// Clears every cached entry for an account, regardless of which session token cached it.
    /// Needed after `link_discord_id_to_account` commits: that update goes straight to the DB
    /// with no session token in hand (the discord callback is state-verified, not bearer-authed),
    /// so a stale pre-link entry would otherwise keep serving the old `discord_name` for up to
    /// `ACCOUNT_AUTH_CACHE_TTL` after the link succeeds.
    pub fn invalidate_account(&self, account_id: i64) {
        let Ok(mut inner) = self.inner.write() else {
            return;
        };
        inner.entries.retain(|_, entry| entry.account.id != account_id);
    }

    fn insert(&self, token_hash: String, account: Account) {
        let Ok(mut inner) = self.inner.write() else {
            return;
        };
        let now = Instant::now();
        if now.duration_since(inner.last_sweep) >= ACCOUNT_AUTH_CACHE_SWEEP_INTERVAL {
            inner.entries.retain(|_, entry| entry.expires_at > now);
            inner.last_sweep = now;
        }
        if inner.entries.len() >= ACCOUNT_AUTH_CACHE_MAX_ENTRIES {
            return;
        }
        inner.entries.insert(
            token_hash,
            CachedAccount {
                account,
                expires_at: now + ACCOUNT_AUTH_CACHE_TTL,
            },
        );
    }
}

impl Default for AccountAuthenticationCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AccountAuthenticateMiddlewareFactory {
    cache: Arc<AccountAuthenticationCache>,
}
impl AccountAuthenticateMiddlewareFactory {
    pub fn new(cache: Arc<AccountAuthenticationCache>) -> Self {
        AccountAuthenticateMiddlewareFactory { cache }
    }
}
impl<S, B> Transform<S, ServiceRequest> for AccountAuthenticateMiddlewareFactory
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: actix_web::body::MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type InitError = ();
    type Transform = AccountAuthenticateMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AccountAuthenticateMiddleware {
            service: Rc::new(service),
            cache: Arc::clone(&self.cache),
        }))
    }
}

type AccountAuthenticationInfo = Rc<Account>;
pub struct AccountAuthenticated(AccountAuthenticationInfo);
impl std::ops::Deref for AccountAuthenticated {
    type Target = Account;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl FromRequest for AccountAuthenticated {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let value = req.extensions().get::<AccountAuthenticationInfo>().cloned();
        let result = match value {
            Some(v) => {
                if v.must_change_password && !must_change_password_allowlisted(req.path()) {
                    Err(ApiError::MustChangePasswordError.into())
                } else {
                    Ok(AccountAuthenticated(v))
                }
            }
            None => Err(actix_web::error::ErrorUnauthorized("")),
        };
        ready(result)
    }
}
pub struct AccountAuthenticateMiddleware<S> {
    service: Rc<S>,
    cache: Arc<AccountAuthenticationCache>,
}

async fn authenticate_via_db(
    req: &ServiceRequest,
    token_hash: &str,
    cache: &AccountAuthenticationCache,
) -> Result<Account, actix_web::Error> {
    let db_pool = req
        .app_data::<web::Data<Pool>>()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError(""))?;
    let client = db_pool
        .get()
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

    let account = db::get_account_by_session_token_hash(&client, token_hash)
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError(""))?
        .ok_or_else(|| actix_web::error::ErrorUnauthorized(""))?;

    // Best-effort - a failed write here shouldn't fail the request it's just piggybacking on.
    let _ = db::record_account_visit(&client, account.id).await;

    cache.insert(
        token_hash.to_owned(),
        Account {
            id: account.id,
            username: account.username.clone(),
            discord_name: account.discord_name.clone(),
            created_at: account.created_at,
            must_change_password: account.must_change_password,
        },
    );
    Ok(account)
}

/// Endpoints reachable even while `must_change_password` is set - just enough for the account
/// page to load and for the password itself to be changed. Everything else under the account
/// scope's authed middleware gets `ApiError::MustChangePasswordError` until the password is
/// changed, which clears the flag.
fn must_change_password_allowlisted(path: &str) -> bool {
    path.ends_with("/account/me") || path.ends_with("/account/password")
}

impl<S, B> Service<ServiceRequest> for AccountAuthenticateMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: actix_web::body::MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let srv = Rc::clone(&self.service);
        let cache = Arc::clone(&self.cache);

        async move {
            let auth_header = match req.headers().get("Authorization") {
                Some(auth_header) => auth_header,
                None => {
                    return Ok(req.error_response(actix_web::error::ErrorUnauthorized(
                        "Authorization header missing from request",
                    )));
                }
            };
            let token = match auth_header.to_str() {
                Ok(token) => token,
                Err(_) => {
                    return Ok(req.error_response(actix_web::error::ErrorBadRequest(
                        "Unable to parse Authorization header",
                    )));
                }
            };

            let token_hash = crate::crypto::session_token_hash(token);
            let account = match cache.get(&token_hash) {
                Some(account) => account,
                None => match authenticate_via_db(&req, &token_hash, &cache).await {
                    Ok(account) => account,
                    Err(e) => return Ok(req.error_response(e)),
                },
            };

            req.extensions_mut()
                .insert::<AccountAuthenticationInfo>(Rc::new(account));

            let res = srv.call(req).await?;
            Ok(res.map_into_boxed_body())
        }
        .boxed_local()
    }
}

#[cfg(test)]
mod tests {
    use super::{Account, AccountAuthenticationCache};
    use chrono::Utc;

    fn test_account() -> Account {
        Account {
            id: 42,
            username: Some("player1".to_string()),
            discord_name: None,
            created_at: Utc::now(),
            must_change_password: false,
        }
    }

    #[test]
    fn caches_successful_authentication_by_token_hash() {
        let cache = AccountAuthenticationCache::new();
        cache.insert("valid-token-hash".to_owned(), test_account());

        assert!(cache.get("valid-token-hash").is_some());
        assert!(cache.get("other-token-hash").is_none());
    }
}
