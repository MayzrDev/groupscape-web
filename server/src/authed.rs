use crate::auth_middleware::Authenticated;
use crate::config::Config;
use crate::crypto::session_token_hash;
use crate::db;
use crate::drop_rates;
use crate::error::ApiError;
use crate::models::{
    ActivityEvent, AmIInGroupRequest, BlockedMember, GameEvent, GroupCredentials, GroupMember,
    GroupMemberName, GroupMemberPermissions, GroupSession, GroupSkillData, LootSplitParticipant,
    LootSplitResult, LootSummaryRow, PermissionFlags, PermissionKey, RenameGroup,
    UpdateGroupPermissionsRequest, SHARED_MEMBER,
};
use crate::permissions::{require_group_permission, ACCOUNT_AUTH_HEADER};
use crate::push;
use crate::unauthed::get_ge_prices_map;
use crate::validators::{valid_name, validate_member_prop_length, ArrayFormat};
use crate::websocket::{self, GroupBroadcastRegistry, VitalsUpdatePayload, WsEnvelope};
use actix_web::{delete, get, post, put, web, Error, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use deadpool_postgres::{Client, Pool};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[delete("/delete-group-member")]
pub async fn delete_group_member(
    req: HttpRequest,
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
    require_group_permission(&req, &client, auth.group_id, PermissionKey::KickMembers).await?;
    db::delete_group_member(&mut client, auth.group_id, &group_member.name).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/block-group-member")]
pub async fn block_group_member(
    req: HttpRequest,
    auth: Authenticated,
    group_member: web::Json<GroupMemberName>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    if group_member.name.eq(SHARED_MEMBER) {
        return Ok(
            HttpResponse::BadRequest().body(format!("Member name {} not allowed", SHARED_MEMBER))
        );
    }

    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::KickMembers).await?;
    db::block_group_member(&mut client, auth.group_id, &group_member.name).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/unblock-group-member")]
pub async fn unblock_group_member(
    req: HttpRequest,
    auth: Authenticated,
    group_member: web::Json<GroupMemberName>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::KickMembers).await?;
    db::unblock_group_member(&client, auth.group_id, &group_member.name).await?;
    Ok(HttpResponse::Ok().finish())
}

/// Self-check for the site's remove/block member controls: a 401/403 here means the acting
/// account can't call `delete-group-member`/`block-group-member`/`unblock-group-member` either,
/// so the site hides those controls rather than showing ones that would just 403 on click.
#[get("/can-kick-members")]
pub async fn can_kick_members(
    req: HttpRequest,
    auth: Authenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::KickMembers).await?;
    Ok(HttpResponse::Ok().finish())
}

#[get("/get-blocked-members")]
pub async fn get_blocked_members(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
) -> Result<web::Json<Vec<BlockedMember>>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let blocked_members = db::get_blocked_members(&client, auth.group_id).await?;
    Ok(web::Json(blocked_members))
}

#[put("/rename-group")]
pub async fn rename_group(
    req: HttpRequest,
    auth: Authenticated,
    rename_group: web::Json<RenameGroup>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let new_name = rename_group.new_name.trim().to_string();
    if !valid_name(&new_name) {
        return Ok(HttpResponse::BadRequest().body("Provided group name is not valid"));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::ManageSettings).await?;
    let new_token = db::rename_group(&client, auth.group_id, &new_name).await?;
    Ok(HttpResponse::Ok().json(&GroupCredentials {
        name: new_name,
        token: new_token,
    }))
}

#[post("/reroll-group-token")]
pub async fn reroll_group_token(
    req: HttpRequest,
    auth: Authenticated,
    path: web::Path<String>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let group_name = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(
        &req,
        &client,
        auth.group_id,
        PermissionKey::RegenerateGroupKey,
    )
    .await?;
    let new_token = db::reroll_group_token(&client, auth.group_id, &group_name).await?;
    Ok(HttpResponse::Ok().json(&GroupCredentials {
        name: group_name,
        token: new_token,
    }))
}

#[delete("/delete-group")]
pub async fn delete_group(
    req: HttpRequest,
    auth: Authenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::ManageSettings).await?;
    db::admin_delete_group(&mut client, auth.group_id).await?;
    Ok(HttpResponse::Ok().finish())
}

/// Doubles as the client-side admin gate for the permission-management UI: only accounts
/// holding `ManagePermissions` (by default, only the group admin, via the implicit override in
/// `has_group_permission`) can call this at all, so the site treats a 401/403 here as "hide the
/// section" rather than needing a separate am-I-admin check.
#[get("/get-group-permissions")]
pub async fn get_group_permissions(
    req: HttpRequest,
    auth: Authenticated,
    db_pool: web::Data<Pool>,
) -> Result<web::Json<Vec<GroupMemberPermissions>>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(
        &req,
        &client,
        auth.group_id,
        PermissionKey::ManagePermissions,
    )
    .await?;
    let permissions = db::list_group_member_permissions(&client, auth.group_id).await?;
    Ok(web::Json(permissions))
}

/// The caller's own effective flags (admin override folded in), keyed off whatever account
/// `X-Account-Authorization` resolves to - unlike `get_group_permissions` this isn't gated by
/// `ManagePermissions`, since every member needs to know their own capabilities to decide which
/// controls to show (e.g. the remove-member button). No/invalid account header or no
/// permissions row all fall back to [`PermissionFlags::default`] (every flag off) rather than
/// erroring, so a logged-out browser just sees nothing gated.
#[get("/get-my-permissions")]
pub async fn get_my_permissions(
    req: HttpRequest,
    auth: Authenticated,
    db_pool: web::Data<Pool>,
) -> Result<web::Json<PermissionFlags>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let account = match req
        .headers()
        .get(ACCOUNT_AUTH_HEADER)
        .and_then(|value| value.to_str().ok())
    {
        Some(token) => {
            db::get_account_by_session_token_hash(&client, &session_token_hash(token)).await?
        }
        None => None,
    };

    let flags = match account {
        Some(account) => {
            db::get_effective_permission_flags(&client, auth.group_id, account.id).await?
        }
        None => PermissionFlags::default(),
    };

    Ok(web::Json(flags))
}

#[put("/update-group-permissions")]
pub async fn update_group_permissions(
    req: HttpRequest,
    auth: Authenticated,
    body: web::Json<UpdateGroupPermissionsRequest>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(
        &req,
        &client,
        auth.group_id,
        PermissionKey::ManagePermissions,
    )
    .await?;
    let body = body.into_inner();
    let updated =
        db::update_group_permissions(&client, auth.group_id, body.account_id, body.patch).await?;
    match updated {
        Some(permissions) => Ok(HttpResponse::Ok().json(permissions)),
        None => Ok(HttpResponse::NotFound().body("Account has no permissions row in this group")),
    }
}

#[post("/update-group-member")]
pub async fn update_group_member(
    auth: Authenticated,
    group_member: web::Json<GroupMember>,
    sender: web::Data<mpsc::Sender<GroupMember>>,
    broadcast_registry: web::Data<GroupBroadcastRegistry>,
    db_pool: web::Data<Pool>,
    config: web::Data<Config>,
) -> Result<HttpResponse, Error> {
    if group_member.name.eq(SHARED_MEMBER) {
        return Ok(
            HttpResponse::BadRequest().body(format!("Member name {} not allowed", SHARED_MEMBER))
        );
    }

    // No manual "add member" step - the group roster is provisioned lazily from
    // telemetry itself. This also enforces the block list and the 5-member cap on
    // brand-new names before anything gets written or broadcast.
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::ensure_group_member(&client, auth.group_id, &group_member.name).await?;

    let mut group_member_inner: GroupMember = group_member.into_inner();
    group_member_inner.group_id = Some(auth.group_id);

    // Derive the canonical member name from the plugin-submitted account_hash rather than
    // trusting the client-supplied name, when the character is linked (account created,
    // character linked, and linked to this specific group). Legacy plugin builds and
    // characters that aren't linked yet fall back to matching an existing member row by name,
    // so group setup can still take a typed RSN in the meantime.
    // Also resolves the account a low-HP/wilderness alert (below) should push to - a character
    // can carry alerts once linked to an account even before it's linked to this specific group.
    let mut linked_account_id: Option<i64> = None;
    if let Some(account_hash) = group_member_inner.account_hash.clone() {
        let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
        if let Some(character) = db::find_character_by_account_hash(&client, &account_hash).await? {
            linked_account_id = Some(character.account_id);
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

    // Discrete kill/death events are written straight through (not routed via the batcher,
    // which only knows latest-value-wins upserts) before the vitals fields hand off to it.
    // `.take()` since the batcher's `GroupMember` merge/dedupe logic has no notion of this
    // field and shouldn't carry it along.
    if let Some(events) = group_member_inner.events.take() {
        if !events.is_empty() {
            let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
            let session_id = db::ensure_open_session(&client, auth.group_id).await?;
            for event in &events {
                db::insert_activity_event(
                    &client,
                    auth.group_id,
                    session_id,
                    &group_member_inner.name,
                    event,
                )
                .await?;
            }
        }
    }

    // NPC dialogue/object-interaction events ride the same heartbeat under their own
    // "interactions"/"object_interactions" keys (see `DialogueEvent`/`ObjectInteractionEvent`),
    // stored into the same generic `activity_events` table as kill/death.
    if let Some(interactions) = group_member_inner.interactions.take() {
        if !interactions.is_empty() {
            let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
            let session_id = db::ensure_open_session(&client, auth.group_id).await?;
            for event in &interactions {
                db::insert_dialogue_event(
                    &client,
                    auth.group_id,
                    session_id,
                    &group_member_inner.name,
                    event,
                )
                .await?;
            }
        }
    }
    if let Some(object_interactions) = group_member_inner.object_interactions.take() {
        if !object_interactions.is_empty() {
            let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
            let session_id = db::ensure_open_session(&client, auth.group_id).await?;
            for event in &object_interactions {
                db::insert_object_interaction_event(
                    &client,
                    auth.group_id,
                    session_id,
                    &group_member_inner.name,
                    event,
                )
                .await?;
            }
        }
    }

    // Low-HP/wilderness-entry alerts are consumed once to trigger a web push (never stored,
    // unlike `events`/`interactions`/`object_interactions` above) - a no-op if the character
    // isn't account-linked yet or push isn't configured on this server.
    if let Some(alerts) = group_member_inner.alerts.take() {
        if let Some(account_id) = linked_account_id {
            for alert in alerts {
                push::dispatch_alert_push(
                    config.push.clone(),
                    db_pool.get_ref().clone(),
                    account_id,
                    group_member_inner.name.clone(),
                    alert,
                );
            }
        }
    }

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

fn default_activity_limit() -> i64 {
    30
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetActivityEventsQuery {
    #[serde(default)]
    pub member_name: Option<String>,
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default)]
    pub before: Option<DateTime<Utc>>,
    #[serde(default = "default_activity_limit")]
    pub limit: i64,
}
#[get("/get-activity-events")]
pub async fn get_activity_events(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetActivityEventsQuery>,
) -> Result<web::Json<Vec<ActivityEvent>>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let events = db::list_activity_events(
        &client,
        auth.group_id,
        query.member_name.as_deref(),
        query.event_type.as_deref(),
        query.before,
        query.limit,
    )
    .await?;
    Ok(web::Json(events))
}

fn default_sessions_limit() -> i64 {
    20
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetSessionsQuery {
    #[serde(default = "default_sessions_limit")]
    pub limit: i64,
}
#[get("/get-sessions")]
pub async fn get_sessions(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetSessionsQuery>,
) -> Result<web::Json<Vec<GroupSession>>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let sessions = db::list_sessions(&client, auth.group_id, query.limit).await?;
    Ok(web::Json(sessions))
}

fn default_loot_sort() -> String {
    "value".to_string()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetLootSummaryQuery {
    #[serde(default)]
    pub member_name: Option<String>,
    #[serde(default = "default_loot_sort")]
    pub sort: String,
}
/// Query-time pivot over `kill` activity events into per-(member, npc, item) rows, joining GE
/// value (in-process price cache, no live wiki refetch) and rarity/uniqueness (curated
/// `drop_rates` table) at read time - mirrors `groupscape-old`'s `GET /loot-summary`, adapted
/// to this server's flat `npc_name` (no `contentKey` slug exists here).
#[get("/get-loot-summary")]
pub async fn get_loot_summary(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetLootSummaryQuery>,
) -> Result<web::Json<Vec<LootSummaryRow>>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let events = db::list_kill_events(
        &client,
        auth.group_id,
        query.member_name.as_deref(),
        None,
        None,
    )
    .await?;
    let ge_prices = get_ge_prices_map();

    let mut rows: HashMap<(String, String, i32), LootSummaryRow> = HashMap::new();
    for event in &events {
        let Ok(GameEvent::Kill(kill)) = serde_json::from_value::<GameEvent>(event.payload.clone())
        else {
            continue;
        };
        let Some(loot) = kill.loot else {
            continue;
        };
        for item in loot {
            let key = (
                event.member_name.clone(),
                kill.npc_name.clone(),
                item.item_id,
            );
            let unit_value = ge_prices.get(&item.item_id).copied();
            let drop = drop_rates::lookup(&kill.npc_name, item.item_id);
            let row = rows.entry(key).or_insert_with(|| LootSummaryRow {
                member_name: event.member_name.clone(),
                npc_name: kill.npc_name.clone(),
                item_id: item.item_id,
                item_name: drop.map(|d| d.name.clone()),
                quantity: 0,
                unit_value,
                total_value: 0,
                rarity: drop.map(|d| d.rarity.clone()),
                is_unique: drop.map(|d| d.is_unique).unwrap_or(false),
            });
            row.quantity += item.quantity;
            row.total_value += unit_value.unwrap_or(0) * item.quantity as i64;
        }
    }

    let mut result: Vec<LootSummaryRow> = rows.into_values().collect();
    match query.sort.as_str() {
        "rarity" => result.sort_by(|a, b| {
            let rank_a = drop_rates::rarity_rank(a.rarity.as_deref().unwrap_or(""));
            let rank_b = drop_rates::rarity_rank(b.rarity.as_deref().unwrap_or(""));
            rank_b.cmp(&rank_a).then(b.total_value.cmp(&a.total_value))
        }),
        _ => result.sort_by(|a, b| b.total_value.cmp(&a.total_value)),
    }
    Ok(web::Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetLootSplitQuery {
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
    #[serde(default)]
    pub until: Option<DateTime<Utc>>,
}
/// Total loot GP over `[since, until]` split evenly across every member who reported a kill in
/// that range. Unlike `groupscape-old`'s split (which credits a per-kill `actor_character_ids`
/// list), this server has no multi-actor kill co-attribution, so "participants" here is the set
/// of reporting members rather than a raid roster.
#[get("/get-loot-split")]
pub async fn get_loot_split(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetLootSplitQuery>,
) -> Result<web::Json<LootSplitResult>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let events =
        db::list_kill_events(&client, auth.group_id, None, query.since, query.until).await?;
    let ge_prices = get_ge_prices_map();

    let mut per_member: HashMap<String, (i64, i64)> = HashMap::new();
    let mut total_value: i64 = 0;
    let kill_count = events.len() as i64;
    for event in &events {
        let entry = per_member
            .entry(event.member_name.clone())
            .or_insert((0, 0));
        entry.0 += 1;
        if let Ok(GameEvent::Kill(kill)) =
            serde_json::from_value::<GameEvent>(event.payload.clone())
        {
            if let Some(loot) = kill.loot {
                for item in loot {
                    let value =
                        ge_prices.get(&item.item_id).copied().unwrap_or(0) * item.quantity as i64;
                    entry.1 += value;
                    total_value += value;
                }
            }
        }
    }

    let mut participants: Vec<LootSplitParticipant> = per_member
        .into_iter()
        .map(|(member_name, (kills, loot_value))| LootSplitParticipant {
            member_name,
            kill_count: kills,
            loot_value,
        })
        .collect();
    participants.sort_by(|a, b| a.member_name.cmp(&b.member_name));

    let participant_count = participants.len() as i64;
    let (per_person_gp, remainder_gp) = if participant_count > 0 {
        (
            total_value / participant_count,
            total_value % participant_count,
        )
    } else {
        (0, 0)
    };

    Ok(web::Json(LootSplitResult {
        total_value,
        kill_count,
        participants,
        per_person_gp,
        remainder_gp,
    }))
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
    // Two dynamic segments are live here: {group_name} from the outer scope plus
    // {member_name} from this route. Path<String> only tolerates a single segment
    // and 404s ("wrong number of parameters: 2 expected 1") otherwise.
    path: web::Path<(String, String)>,
    body: web::Bytes,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let (_group_name, member_name) = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let member_id = db::get_member_id(&client, auth.group_id, &member_name).await?;
    db::upsert_member_mesh(&client, member_id, &body).await?;
    Ok(HttpResponse::Ok().finish())
}

#[get("/portrait/{member_name}")]
pub async fn get_portrait(
    auth: Authenticated,
    path: web::Path<(String, String)>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let (_group_name, member_name) = path.into_inner();
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
