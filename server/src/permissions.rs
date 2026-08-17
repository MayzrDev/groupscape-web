use crate::crypto::session_token_hash;
use crate::db;
use crate::error::ApiError;
use crate::models::PermissionKey;
use actix_web::HttpRequest;
use deadpool_postgres::Client;

/// Carries the acting account's session token on group-token-authenticated (`/api/group/...`)
/// requests. Separate header from `Authorization`, which on this scope identifies a *group*
/// (shared token used by the RuneLite plugin and logged-out site sessions) rather than an
/// account - permission-gated actions need both to know who is acting.
pub const ACCOUNT_AUTH_HEADER: &str = "X-Account-Authorization";

/// Resolves the acting account from `X-Account-Authorization` and checks it holds `key` in
/// `group_id`, folding in the group admin's implicit all-permissions override via
/// `db::has_group_permission`. Returns the account id on success so callers can use it (e.g.
/// for audit logging) without a second lookup.
pub async fn require_group_permission(
    req: &HttpRequest,
    client: &Client,
    group_id: i64,
    key: PermissionKey,
) -> Result<i64, ApiError> {
    let token = req
        .headers()
        .get(ACCOUNT_AUTH_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::AccountAuthRequiredError)?;

    let token_hash = session_token_hash(token);
    let account = db::get_account_by_session_token_hash(client, &token_hash)
        .await?
        .ok_or(ApiError::AccountAuthRequiredError)?;

    if db::has_group_permission(client, group_id, account.id, key).await? {
        Ok(account.id)
    } else {
        Err(ApiError::PermissionDeniedError)
    }
}
