use crate::admin_auth_middleware::AdminAuthenticated;
use crate::db;
use crate::error::ApiError;
use crate::models::{
    AdminAccountsSummary, AdminAuditLogResponse, AdminFeatureFlag, AdminGroupsQuery,
    AdminGroupsResponse, AdminModerationRequest, AdminPageQuery, AdminSetFeatureFlag,
};
use actix_web::{delete, get, post, put, web, Error, HttpResponse};
use deadpool_postgres::{Client, Pool};
use serde_json::json;

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
