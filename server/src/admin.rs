use crate::admin_auth_middleware::AdminAuthenticated;
use crate::crypto;
use crate::db;
use crate::error::ApiError;
use crate::models::{
    AdminAccountsQuery, AdminAccountsResponse, AdminAccountsSummary, AdminAuditLogResponse,
    AdminFeatureFlag, AdminGroupsQuery, AdminGroupsResponse, AdminModerationRequest,
    AdminPageQuery, AdminPasswordResetResponse, AdminSearchQuery, AdminSearchResponse,
    AdminSetAccountUsername, AdminSetAccountStatus, AdminSetFeatureFlag,
};
use actix_web::{delete, get, post, put, web, Error, HttpResponse};
use deadpool_postgres::{Client, Pool};
use serde_json::json;

const VALID_ACCOUNT_STATUSES: &[&str] = &["active", "suspended", "banned", "deleted"];

/// Ownership-transfer + membership-removal cascade shared by ban and hard-delete: every group
/// this account owns gets a new admin (or `NULL` if it was the last member), matching
/// `db::transfer_or_clear_group_ownership`'s "unclaimed" semantics.
async fn cascade_transfer_owned_groups(client: &Client, account_id: i64) -> Result<(), ApiError> {
    let owned_group_ids = db::admin_get_owned_group_ids(client, account_id).await?;
    for group_id in owned_group_ids {
        db::transfer_or_clear_group_ownership(client, group_id, account_id).await?;
    }
    Ok(())
}

#[get("/am-i-logged-in")]
pub async fn am_i_logged_in(_auth: AdminAuthenticated) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().finish())
}

#[get("/groups")]
pub async fn list_groups(
    _auth: AdminAuthenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<AdminGroupsQuery>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let (groups, total) = db::admin_list_groups(
        &client,
        query.search.as_deref(),
        query.page,
        query.page_size,
    )
    .await?;
    Ok(HttpResponse::Ok().json(AdminGroupsResponse { groups, total }))
}

#[get("/groups/{group_id}")]
pub async fn get_group(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let group_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    match db::admin_get_group(&client, group_id).await? {
        Some(detail) => Ok(HttpResponse::Ok().json(detail)),
        None => Err(ApiError::AdminNotFoundError.into()),
    }
}

async fn set_group_moderation(
    db_pool: &web::Data<Pool>,
    group_id: i64,
    status: &str,
    reason: Option<&str>,
    action: &str,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_group(&client, group_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    db::admin_set_group_moderation(&client, group_id, status, reason).await?;
    db::admin_record_audit_log(
        &client,
        action,
        Some("group"),
        Some(&group_id.to_string()),
        Some(json!({ "reason": reason, "status": status })),
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/groups/{group_id}/suspend")]
pub async fn suspend_group(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    body: web::Json<AdminModerationRequest>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    set_group_moderation(
        &db_pool,
        path.into_inner(),
        "suspended",
        body.reason.as_deref(),
        "group.suspend",
    )
    .await
}

#[post("/groups/{group_id}/ban")]
pub async fn ban_group(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    body: web::Json<AdminModerationRequest>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    set_group_moderation(
        &db_pool,
        path.into_inner(),
        "banned",
        body.reason.as_deref(),
        "group.ban",
    )
    .await
}

#[post("/groups/{group_id}/unban")]
pub async fn unban_group(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    set_group_moderation(&db_pool, path.into_inner(), "active", None, "group.unban").await
}

#[delete("/groups/{group_id}")]
pub async fn delete_group(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let group_id = path.into_inner();
    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_group(&client, group_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    db::admin_delete_group(&mut client, group_id).await?;
    db::admin_record_audit_log(
        &client,
        "group.delete",
        Some("group"),
        Some(&group_id.to_string()),
        None,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[get("/feature-flags")]
pub async fn list_feature_flags(
    _auth: AdminAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let flags = db::admin_list_feature_flags(&client).await?;
    Ok(HttpResponse::Ok().json(flags))
}

#[put("/feature-flags/{flag_key}")]
pub async fn set_feature_flag(
    _auth: AdminAuthenticated,
    path: web::Path<String>,
    body: web::Json<AdminSetFeatureFlag>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let flag_key = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::admin_set_feature_flag(
        &client,
        &flag_key,
        body.enabled,
        body.description.as_deref(),
    )
    .await?;
    db::admin_record_audit_log(
        &client,
        "flag.toggle",
        Some("flag"),
        Some(&flag_key),
        Some(json!({ "enabled": body.enabled })),
    )
    .await?;
    Ok(HttpResponse::Ok().json(AdminFeatureFlag {
        flag_key,
        enabled: body.enabled,
        description: body.description.clone(),
    }))
}

#[get("/audit-log")]
pub async fn list_audit_log(
    _auth: AdminAuthenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<AdminPageQuery>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let (entries, total) = db::admin_list_audit_log(&client, query.page, query.page_size).await?;
    Ok(HttpResponse::Ok().json(AdminAuditLogResponse { entries, total }))
}

#[get("/accounts/summary")]
pub async fn accounts_summary(
    _auth: AdminAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let count = db::admin_count_accounts(&client).await?;
    Ok(HttpResponse::Ok().json(AdminAccountsSummary { count }))
}

#[get("/accounts")]
pub async fn list_accounts(
    _auth: AdminAuthenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<AdminAccountsQuery>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let (accounts, total) = db::admin_list_accounts(
        &client,
        query.search.as_deref(),
        query.status.as_deref(),
        query.group_id,
        query.page,
        query.page_size,
    )
    .await?;
    Ok(HttpResponse::Ok().json(AdminAccountsResponse { accounts, total }))
}

#[get("/accounts/{account_id}")]
pub async fn get_account(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    match db::admin_get_account(&client, account_id).await? {
        Some(detail) => Ok(HttpResponse::Ok().json(detail)),
        None => Err(ApiError::AdminNotFoundError.into()),
    }
}

/// Generates a one-time temp password, forces a change on next login, and revokes every
/// existing session so a leaked/forgotten-password account can't keep using an old one. The
/// temp password is returned once in the response body; the audit log entry deliberately never
/// includes it.
#[post("/accounts/{account_id}/reset-password")]
pub async fn reset_password(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_account(&client, account_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    let temp_password = crypto::generate_temp_password();
    let password_hash =
        crypto::hash_password(&temp_password).map_err(|_| ApiError::InvalidCredentialsError)?;
    db::admin_reset_account_password(&client, account_id, &password_hash).await?;
    db::admin_revoke_all_account_sessions(&client, account_id).await?;
    db::admin_record_audit_log(
        &client,
        "account.password_reset",
        Some("account"),
        Some(&account_id.to_string()),
        None,
    )
    .await?;

    Ok(HttpResponse::Ok().json(AdminPasswordResetResponse { temp_password }))
}

#[post("/accounts/{account_id}/status")]
pub async fn set_account_status(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    body: web::Json<AdminSetAccountStatus>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    if !VALID_ACCOUNT_STATUSES.contains(&body.status.as_str()) {
        return Ok(HttpResponse::BadRequest().body("Unknown account status"));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_account(&client, account_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    db::admin_set_account_status(&client, account_id, &body.status).await?;
    if body.status == "banned" {
        cascade_transfer_owned_groups(&client, account_id).await?;
        db::admin_remove_all_group_memberships(&client, account_id).await?;
        db::admin_revoke_all_account_sessions(&client, account_id).await?;
    }

    db::admin_record_audit_log(
        &client,
        "account.status_changed",
        Some("account"),
        Some(&account_id.to_string()),
        Some(json!({ "status": body.status })),
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}

/// Reversible: status flips to `deleted` and the username is scrubbed, but group memberships are
/// left alone (unlike ban/hard-delete) so an admin can restore the account later by flipping
/// status back and setting a fresh username.
#[post("/accounts/{account_id}/soft-delete")]
pub async fn soft_delete_account(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_account(&client, account_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    db::admin_soft_delete_account(&client, account_id).await?;
    db::admin_revoke_all_account_sessions(&client, account_id).await?;
    db::admin_record_audit_log(
        &client,
        "account.soft_deleted",
        Some("account"),
        Some(&account_id.to_string()),
        None,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}

/// Irreversible: transfers ownership of any owned groups, then hard-deletes the account row -
/// `characters`, `character_group_links`, `group_permissions`, and `account_sessions` all clean
/// up via `ON DELETE CASCADE` off `accounts(id)`.
#[post("/accounts/{account_id}/hard-delete")]
pub async fn hard_delete_account(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_account(&client, account_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    cascade_transfer_owned_groups(&client, account_id).await?;
    db::delete_account(&client, account_id).await?;
    db::admin_record_audit_log(
        &client,
        "account.hard_deleted",
        Some("account"),
        Some(&account_id.to_string()),
        None,
    )
    .await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/accounts/{account_id}/sessions")]
pub async fn list_account_sessions(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_account(&client, account_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    let sessions = db::admin_list_account_sessions(&client, account_id).await?;
    Ok(HttpResponse::Ok().json(sessions))
}

#[post("/accounts/{account_id}/sessions/{session_id}/revoke")]
pub async fn revoke_account_session(
    _auth: AdminAuthenticated,
    path: web::Path<(i64, i64)>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let (account_id, session_id) = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let revoked = db::admin_revoke_account_session(&client, account_id, session_id).await?;
    if !revoked {
        return Err(ApiError::AdminNotFoundError.into());
    }

    db::admin_record_audit_log(
        &client,
        "session.revoked",
        Some("account_session"),
        Some(&session_id.to_string()),
        Some(json!({ "account_id": account_id })),
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/accounts/{account_id}/sessions/revoke-all")]
pub async fn revoke_all_account_sessions(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_account(&client, account_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    db::admin_revoke_all_account_sessions(&client, account_id).await?;
    db::admin_record_audit_log(
        &client,
        "session.revoked",
        Some("account"),
        Some(&account_id.to_string()),
        Some(json!({ "all": true })),
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/accounts/{account_id}/username")]
pub async fn set_account_username(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    body: web::Json<AdminSetAccountUsername>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    let username = body.username.trim().to_string();
    if !crate::validators::valid_name(&username) {
        return Ok(HttpResponse::BadRequest().body("Provided username is not valid"));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_account(&client, account_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    match db::update_account_username(&client, account_id, &username).await {
        Ok(()) => {}
        Err(ApiError::UsernameAlreadyRegisteredError) => {
            return Ok(HttpResponse::Conflict().body("Username already registered"));
        }
        Err(err) => return Err(err.into()),
    }

    db::admin_record_audit_log(
        &client,
        "account.username_changed",
        Some("account"),
        Some(&account_id.to_string()),
        None,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/accounts/{account_id}/clear-lockout")]
pub async fn clear_account_lockout(
    _auth: AdminAuthenticated,
    path: web::Path<i64>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    if db::admin_get_account(&client, account_id).await?.is_none() {
        return Err(ApiError::AdminNotFoundError.into());
    }

    db::admin_clear_account_lockout(&client, account_id).await?;
    db::admin_record_audit_log(
        &client,
        "account.lockout_cleared",
        Some("account"),
        Some(&account_id.to_string()),
        None,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[get("/search")]
pub async fn search(
    _auth: AdminAuthenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<AdminSearchQuery>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let (accounts, groups) = db::admin_search(&client, query.q.trim()).await?;
    Ok(HttpResponse::Ok().json(AdminSearchResponse { accounts, groups }))
}

#[get("/dashboard")]
pub async fn dashboard(
    _auth: AdminAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let dashboard = db::admin_dashboard(&client).await?;
    Ok(HttpResponse::Ok().json(dashboard))
}
