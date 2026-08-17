use crate::auth_middleware::Authenticated;
use crate::db;
use crate::error::ApiError;
use crate::models::{
    AmIInGroupRequest, GroupCredentials, GroupMember, GroupSkillData, RenameGroup,
    RenameGroupMember, SHARED_MEMBER,
};
use crate::validators::{valid_name, validate_member_prop_length, ArrayFormat};
use crate::websocket::{self, GroupBroadcastRegistry, VitalsUpdatePayload, WsEnvelope};
use actix_web::{delete, get, post, put, web, Error, HttpResponse};
use chrono::{DateTime, Utc};
use deadpool_postgres::{Client, Pool};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[post("/add-group-member")]
pub async fn add_group_member(
    auth: Authenticated,
    group_member: web::Json<GroupMember>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    if group_member.name.eq(SHARED_MEMBER) {
        return Ok(
            HttpResponse::BadRequest().body(format!("Member name {} not allowed", SHARED_MEMBER))
        );
    }

    if !valid_name(&group_member.name) {
        return Ok(HttpResponse::BadRequest()
            .body(format!("Member name {} is not valid", group_member.name)));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::add_group_member(&client, auth.group_id, &group_member.name).await?;
    Ok(HttpResponse::Created().finish())
}

#[delete("/delete-group-member")]
pub async fn delete_group_member(
    auth: Authenticated,
    group_member: web::Json<GroupMember>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    if group_member.name.eq(SHARED_MEMBER) {
        return Ok(
            HttpResponse::BadRequest().body(format!("Member name {} not allowed", SHARED_MEMBER))
        );
    }

    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::delete_group_member(&mut client, auth.group_id, &group_member.name).await?;
    Ok(HttpResponse::Ok().finish())
}

#[put("/rename-group-member")]
pub async fn rename_group_member(
    auth: Authenticated,
    rename_member: web::Json<RenameGroupMember>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    if rename_member.original_name.eq(SHARED_MEMBER) || rename_member.new_name.eq(SHARED_MEMBER) {
        return Ok(
            HttpResponse::BadRequest().body(format!("Member name {} not allowed", SHARED_MEMBER))
        );
    }

    if !valid_name(&rename_member.new_name) {
        return Ok(HttpResponse::BadRequest().body(format!(
            "Member name {} is not valid",
            rename_member.new_name
        )));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::rename_group_member(
        &client,
        auth.group_id,
        &rename_member.original_name,
        &rename_member.new_name,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[put("/rename-group")]
pub async fn rename_group(
    auth: Authenticated,
    rename_group: web::Json<RenameGroup>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let new_name = rename_group.new_name.trim().to_string();
    if !valid_name(&new_name) {
        return Ok(HttpResponse::BadRequest().body("Provided group name is not valid"));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let new_token = db::rename_group(&client, auth.group_id, &new_name).await?;
    Ok(HttpResponse::Ok().json(&GroupCredentials {
        name: new_name,
        token: new_token,
    }))
}

#[post("/reroll-group-token")]
pub async fn reroll_group_token(
    auth: Authenticated,
    path: web::Path<String>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let group_name = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let new_token = db::reroll_group_token(&client, auth.group_id, &group_name).await?;
    Ok(HttpResponse::Ok().json(&GroupCredentials {
        name: group_name,
        token: new_token,
    }))
}

#[delete("/delete-group")]
pub async fn delete_group(auth: Authenticated, db_pool: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::admin_delete_group(&mut client, auth.group_id).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/update-group-member")]
pub async fn update_group_member(
    auth: Authenticated,
    group_member: web::Json<GroupMember>,
    sender: web::Data<mpsc::Sender<GroupMember>>,
    broadcast_registry: web::Data<GroupBroadcastRegistry>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    if group_member.name.eq(SHARED_MEMBER) {
        return Ok(
            HttpResponse::BadRequest().body(format!("Member name {} not allowed", SHARED_MEMBER))
        );
    }

    let mut group_member_inner: GroupMember = group_member.into_inner();
    group_member_inner.group_id = Some(auth.group_id);

    // Derive the canonical member name from the plugin-submitted account_hash rather than
    // trusting the client-supplied name, when the character is linked (account created,
    // character linked, and linked to this specific group). Legacy plugin builds and
    // characters that aren't linked yet fall back to matching an existing member row by name,
    // so group setup can still take a typed RSN in the meantime.
    if let Some(account_hash) = group_member_inner.account_hash.clone() {
        let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
        if let Some(character) = db::find_character_by_account_hash(&client, &account_hash).await? {
            let linked_to_this_group = db::find_character_group_link(&client, character.id)
                .await?
                .is_some_and(|link| link.group_id == auth.group_id);
            if linked_to_this_group {
                group_member_inner.name = db::ensure_member_for_linked_character(
                    &client,
                    auth.group_id,
                    &account_hash,
                    &character.display_rsn,
                )
                .await?;
            }
        }
    }

    validate_member_prop_length("stats", &group_member_inner.stats, 7, 7, ArrayFormat::Flat)?;
    validate_member_prop_length(
        "coordinates",
        &group_member_inner.coordinates,
        3,
        4,
        ArrayFormat::Flat,
    )?;
    validate_member_prop_length(
        "skills",
        &group_member_inner.skills,
        23,
        24,
        ArrayFormat::Flat,
    )?;
    validate_member_prop_length(
        "quests",
        &group_member_inner.quests,
        0,
        250,
        ArrayFormat::Flat,
    )?;
    validate_member_prop_length(
        "inventory",
        &group_member_inner.inventory,
        56,
        56,
        ArrayFormat::ItemPairs,
    )?;
    validate_member_prop_length(
        "equipment",
        &group_member_inner.equipment,
        28,
        28,
        ArrayFormat::ItemPairs,
    )?;
    validate_member_prop_length(
        "bank",
        &group_member_inner.bank,
        0,
        3000,
        ArrayFormat::ItemPairs,
    )?;
    validate_member_prop_length(
        "shared_bank",
        &group_member_inner.shared_bank,
        0,
        1000,
        ArrayFormat::ItemPairs,
    )?;
    validate_member_prop_length(
        "rune_pouch",
        &group_member_inner.rune_pouch,
        6,
        8,
        ArrayFormat::ItemPairs,
    )?;
    validate_member_prop_length(
        "seed_vault",
        &group_member_inner.seed_vault,
        0,
        500,
        ArrayFormat::ItemPairs,
    )?;
    validate_member_prop_length(
        "deposited",
        &group_member_inner.deposited,
        0,
        200,
        ArrayFormat::ItemPairs,
    )?;
    validate_member_prop_length(
        "diary_vars",
        &group_member_inner.diary_vars,
        0,
        62,
        ArrayFormat::Flat,
    )?;
    validate_member_prop_length(
        "collection_log_v2",
        &group_member_inner.collection_log_v2,
        0,
        4000,
        ArrayFormat::Flat,
    )?;
    validate_member_prop_length(
        "potion_storage",
        &group_member_inner.potion_storage,
        0,
        400,
        ArrayFormat::ItemPairs,
    )?;

    // Publish straight to any connected party overlays before handing off to
    // the batched DB writer - the batcher trades latency for write
    // efficiency, but the overlay wants these updates as fast as possible.
    if broadcast_registry.has_subscribers(auth.group_id) {
        let envelope = WsEnvelope::VitalsUpdate {
            payload: VitalsUpdatePayload {
                name: group_member_inner.name.clone(),
                vitals: websocket::to_wire_vitals(&group_member_inner),
            },
            ts: Utc::now(),
        };
        if let Ok(message) = serde_json::to_string(&envelope) {
            broadcast_registry.publish(auth.group_id, message);
        }
    }

    match sender.send(group_member_inner).await {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(_) => Ok(HttpResponse::InternalServerError().body("Failed to submit player update")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetGroupDataQuery {
    pub from_time: DateTime<Utc>,
}
#[get("/get-group-data")]
pub async fn get_group_data(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetGroupDataQuery>,
) -> Result<web::Json<Vec<GroupMember>>, Error> {
    let from_time = query.from_time;
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let group_members = db::get_group_data(&client, auth.group_id, &from_time).await?;
    Ok(web::Json(group_members))
}

#[derive(Deserialize)]
pub enum SkillDataPeriod {
    Day,
    Week,
    Month,
    Year,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetSkillDataQuery {
    pub period: SkillDataPeriod,
}
#[get("/get-skill-data")]
pub async fn get_skill_data(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetSkillDataQuery>,
) -> Result<web::Json<GroupSkillData>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let aggregate_period = match query.period {
        SkillDataPeriod::Day => db::AggregatePeriod::Day,
        SkillDataPeriod::Week => db::AggregatePeriod::Month,
        SkillDataPeriod::Month => db::AggregatePeriod::Month,
        SkillDataPeriod::Year => db::AggregatePeriod::Year,
    };
    let group_skill_data =
        db::get_skills_for_period(&client, auth.group_id, aggregate_period).await?;
    Ok(web::Json(group_skill_data))
}

#[get("/am-i-logged-in")]
pub async fn am_i_logged_in(_auth: Authenticated) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().finish())
}

#[get("/am-i-in-group")]
pub async fn am_i_in_group(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    q: web::Query<AmIInGroupRequest>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let in_group: bool = db::is_member_in_group(&client, auth.group_id, &q.member_name).await?;

    if !in_group {
        return Ok(HttpResponse::Unauthorized().body("Player is not a member of this group"));
    }
    Ok(HttpResponse::Ok().finish())
}

#[get("/collection-log")]
pub async fn get_collection_log() -> Result<web::Json<HashMap<String, Vec<i32>>>, Error> {
    Ok(web::Json(HashMap::new()))
}

// Not decorated with #[post(...)] - registered manually in main.rs with its own
// larger PayloadConfig since the global 100KB cap rejects real meshes.
pub async fn update_portrait(
    auth: Authenticated,
    path: web::Path<String>,
    body: web::Bytes,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let member_name = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let member_id = db::get_member_id(&client, auth.group_id, &member_name).await?;
    db::upsert_member_mesh(&client, member_id, &body).await?;
    Ok(HttpResponse::Ok().finish())
}

#[get("/portrait/{member_name}")]
pub async fn get_portrait(
    auth: Authenticated,
    path: web::Path<String>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let member_name = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let mesh = db::get_member_mesh(&client, auth.group_id, &member_name).await?;
    match mesh {
        Some(mesh) => Ok(HttpResponse::Ok()
            .append_header(("Cache-Control", "private, max-age=60"))
            .content_type("application/octet-stream")
            .body(mesh)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}
