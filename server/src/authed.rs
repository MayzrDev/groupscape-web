use crate::auth_middleware::Authenticated;
use crate::character_auth_middleware::CharacterIdentified;
use crate::config::Config;
use crate::crypto::session_token_hash;
use crate::db;
use crate::discord;
use crate::drop_rates;
use crate::error::ApiError;
use crate::item_bonuses;
use crate::leaderboard::{LeaderboardMetric, LeaderboardResult, LeaderboardWindow};
use crate::models::{
    ActivityEvent, AmIInGroupRequest, BlockedMember, DiscordWebhookSettings, GameEvent,
    GroupCredentials, GroupMember, GroupMemberName, GroupMemberPermissions, GroupMetricData,
    GroupSession, GroupSkillData, IdentifyCharacter, ItemBonusesResponse, LootItem,
    LootSplitParticipant, LootSplitResult, LootSummaryRow, MyPermissions, PermissionFlags,
    PermissionKey, RenameGroup,
    UpdateGroupPermissionsRequest, UpdateMemberColorRequest, SHARED_MEMBER,
};
use crate::permissions::{require_any_group_permission, require_group_permission, ACCOUNT_AUTH_HEADER};
use crate::progress_events;
use crate::push;
use crate::update_batcher;
use crate::unauthed::get_ge_prices_map;
use crate::validators::{valid_name, validate_member_prop_length, ArrayFormat};
use crate::websocket::{
    self, DropEventPayload, GroupBroadcastRegistry, KillEventPayload, VitalsUpdatePayload, WsEnvelope,
};
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
    crate::demo::reject_if_demo(auth.group_id)?;
    if group_member.name.eq(SHARED_MEMBER) {
        return Ok(
            HttpResponse::BadRequest().body(format!("Member name {} not allowed", SHARED_MEMBER))
        );
    }

    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let account_id =
        require_group_permission(&req, &client, auth.group_id, PermissionKey::KickMembers).await?;
    if db::resolve_member_name_for_account(&client, auth.group_id, account_id)
        .await?
        .as_deref()
        == Some(group_member.name.as_str())
    {
        return Err(ApiError::CannotRemoveSelfError.into());
    }
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
    crate::demo::reject_if_demo(auth.group_id)?;
    if group_member.name.eq(SHARED_MEMBER) {
        return Ok(
            HttpResponse::BadRequest().body(format!("Member name {} not allowed", SHARED_MEMBER))
        );
    }

    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let account_id =
        require_group_permission(&req, &client, auth.group_id, PermissionKey::KickMembers).await?;
    if db::resolve_member_name_for_account(&client, auth.group_id, account_id)
        .await?
        .as_deref()
        == Some(group_member.name.as_str())
    {
        return Err(ApiError::CannotBlockSelfError.into());
    }
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
    crate::demo::reject_if_demo(auth.group_id)?;
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
    crate::demo::reject_if_demo(auth.group_id)?;
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
    crate::demo::reject_if_demo(auth.group_id)?;
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
    crate::demo::reject_if_demo(auth.group_id)?;
    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::ManageSettings).await?;
    db::admin_delete_group(&mut client, auth.group_id).await?;
    Ok(HttpResponse::Ok().finish())
}

/// Doubles as the client-side admin gate for the member-roster UI: accounts holding
/// `ManagePermissions` (permission toggles) or `ManageSettings` (helmet-colour assignment) can
/// call this - either implies enough to see *some* control in this section, so the site treats a
/// 401/403 here as "hide the section" rather than needing a separate am-I-admin check. Which
/// controls a given caller actually gets to edit is decided client-side from `/get-my-permissions`.
#[get("/get-group-permissions")]
pub async fn get_group_permissions(
    req: HttpRequest,
    auth: Authenticated,
    db_pool: web::Data<Pool>,
) -> Result<web::Json<Vec<GroupMemberPermissions>>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_any_group_permission(
        &req,
        &client,
        auth.group_id,
        &[PermissionKey::ManagePermissions, PermissionKey::ManageSettings],
    )
    .await?;
    let permissions = db::list_group_member_permissions(&client, auth.group_id).await?;
    Ok(web::Json(permissions))
}

/// The caller's own effective flags (admin override folded in) plus their own member name in
/// this group, keyed off whatever account `X-Account-Authorization` resolves to - unlike
/// `get_group_permissions` this isn't gated by `ManagePermissions`, since every member needs to
/// know their own capabilities to decide which controls to show (e.g. the remove-member button,
/// and `member_name` lets the client hide that button on its own row - the server rejects
/// self-removal too, see `delete_group_member`/`block_group_member`). No/invalid account header
/// or no permissions row all fall back to [`PermissionFlags::default`] (every flag off) and a
/// `None` member name, rather than erroring, so a logged-out browser just sees nothing gated.
#[get("/get-my-permissions")]
pub async fn get_my_permissions(
    req: HttpRequest,
    auth: Authenticated,
    db_pool: web::Data<Pool>,
) -> Result<web::Json<MyPermissions>, Error> {
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

    let (flags, member_name) = match account {
        Some(account) => {
            let flags =
                db::get_effective_permission_flags(&client, auth.group_id, account.id).await?;
            let member_name =
                db::resolve_member_name_for_account(&client, auth.group_id, account.id).await?;
            (flags, member_name)
        }
        None => (PermissionFlags::default(), None),
    };

    Ok(web::Json(MyPermissions { member_name, flags }))
}

#[put("/update-group-permissions")]
pub async fn update_group_permissions(
    req: HttpRequest,
    auth: Authenticated,
    body: web::Json<UpdateGroupPermissionsRequest>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    crate::demo::reject_if_demo(auth.group_id)?;
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

/// Assigns a member's helmet/accent colour. Gated on `ManageSettings` rather than
/// `ManagePermissions` - deliberately a different flag from `update_group_permissions`, so an
/// account with settings access but not permissions access can still restyle the roster.
#[put("/update-member-color")]
pub async fn update_member_color(
    req: HttpRequest,
    auth: Authenticated,
    body: web::Json<UpdateMemberColorRequest>,
    db_pool: web::Data<Pool>,
    broadcast_registry: web::Data<GroupBroadcastRegistry>,
) -> Result<HttpResponse, Error> {
    crate::demo::reject_if_demo(auth.group_id)?;
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::ManageSettings).await?;
    let body = body.into_inner();
    let (member_name, updated) =
        db::update_member_color(&client, auth.group_id, body.account_id, &body.color).await?;

    // Push the new colour straight to any connected party overlays so the plugin's accent
    // stripe updates without waiting for a reconnect/resync of the roster snapshot.
    if let Some(hex) = crate::models::named_color_to_hex(&body.color) {
        let envelope = WsEnvelope::ColorUpdate {
            payload: websocket::ColorUpdatePayload {
                name: member_name,
                color: hex,
            },
            ts: Utc::now(),
        };
        if let Ok(message) = serde_json::to_string(&envelope) {
            broadcast_registry.publish(auth.group_id, message);
        }
    }

    Ok(HttpResponse::Ok().json(updated))
}

/// Gated on `ManageDiscord` (kept separate from `ManageSettings` since the webhook URL is a
/// credential, matching `groupscape-old`'s `discordSettings.ts` decision) - the `GET` doubles as
/// the client-side admin gate for the settings page, same pattern as `get_group_permissions`.
#[get("/get-discord-settings")]
pub async fn get_discord_settings(
    req: HttpRequest,
    auth: Authenticated,
    db_pool: web::Data<Pool>,
) -> Result<web::Json<DiscordWebhookSettings>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::ManageDiscord).await?;
    let settings = db::get_discord_webhook_settings(&client, auth.group_id).await?;
    Ok(web::Json(settings))
}

/// Always submits full settings state (not a patch) - see [`DiscordWebhookSettings`]. A
/// non-empty `webhook_url` is validated with a live test message before saving, so a typo'd URL
/// never gets silently stored; an empty/`None` URL clears it without a test call.
#[put("/update-discord-settings")]
pub async fn update_discord_settings(
    req: HttpRequest,
    auth: Authenticated,
    body: web::Json<DiscordWebhookSettings>,
    db_pool: web::Data<Pool>,
) -> Result<web::Json<DiscordWebhookSettings>, Error> {
    crate::demo::reject_if_demo(auth.group_id)?;
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    require_group_permission(&req, &client, auth.group_id, PermissionKey::ManageDiscord).await?;

    let mut settings = body.into_inner();
    settings.webhook_url = settings
        .webhook_url
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty());
    if let Some(url) = &settings.webhook_url {
        discord::test_webhook(url).await?;
    }

    db::update_discord_webhook_settings(&client, auth.group_id, &settings).await?;
    Ok(web::Json(settings))
}

pub async fn update_group_member(
    auth: Authenticated,
    group_member: web::Json<GroupMember>,
    sender: web::Data<mpsc::Sender<GroupMember>>,
    broadcast_registry: web::Data<GroupBroadcastRegistry>,
    db_pool: web::Data<Pool>,
    config: web::Data<Config>,
) -> Result<HttpResponse, Error> {
    crate::demo::reject_if_demo(auth.group_id)?;
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

    // Derive the canonical member name from the account_hash rather than trusting the
    // client-supplied name, when the character is linked (account created, character linked, and
    // linked to this specific group). Prefer the server-resolved account_hash from the
    // character-key auth middleware (`auth.account_hash`) over the client-supplied JSON field -
    // the plugin's real payloads never populate the latter (it only ever sends account_hash in
    // the URL, not the body), so relying on the JSON field left every member row's account_hash
    // permanently NULL and broke anything keyed off it (e.g. the group panel's portrait lookup).
    // Legacy plugin builds and characters that aren't linked yet fall back to matching an
    // existing member row by name, so group setup can still take a typed RSN in the meantime.
    // Also resolves the account a low-HP/wilderness alert (below) should push to - a character
    // can carry alerts once linked to an account even before it's linked to this specific group.
    let mut linked_account_id: Option<i64> = None;
    if let Some(account_hash) = auth
        .account_hash
        .clone()
        .or_else(|| group_member_inner.account_hash.clone())
    {
        let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
        let character = match auth.character_id {
            Some(character_id) => db::find_character_by_id(&client, character_id).await?,
            // Legacy plugin builds and the client-supplied JSON fallback don't carry a resolved
            // character_id, so fall back to whichever account has this account_hash linked to
            // this specific group (unambiguous even if multiple accounts share the account_hash).
            None => {
                db::find_character_by_account_hash_and_group(&client, &account_hash, auth.group_id)
                    .await?
            }
        };
        if let Some(character) = character {
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
        5,
        ArrayFormat::Flat,
    )?;

    if let Some(coordinates) = group_member_inner.coordinates.as_deref() {
        if coordinates.len() >= 5 {
            let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
            db::record_location_sample(
                &client,
                auth.group_id,
                &group_member_inner.name,
                Utc::now(),
                coordinates[4],
                coordinates[2],
                coordinates[0],
                coordinates[1],
            )
            .await?;
        }
    }
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

    // Herb/tree patches (16 in the v1 table) + 4 bird house spaces = 20 possible rows; a
    // generous cap well above that guards against a malformed/malicious payload without needing
    // to track the exact table size here.
    if group_member_inner.farming_timers.as_ref().is_some_and(|v| v.len() > 64) {
        return Ok(HttpResponse::BadRequest().body("farming_timers exceeds max length"));
    }

    // Farming/bird house timers live in their own table (not the batcher's coalesced columns) so
    // the completion-push job can query by ready_at - written straight through here, same as
    // `events` below. `.take()` so the batcher's merge/dedupe logic doesn't have to carry a field
    // it has no notion of.
    if let Some(farming_timers) = group_member_inner.farming_timers.take() {
        let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
        db::replace_farming_timers(&mut client, auth.group_id, &group_member_inner.name, &farming_timers)
            .await?;
    }

    // Discrete kill/death events are written straight through (not routed via the batcher,
    // which only knows latest-value-wins upserts) before the vitals fields hand off to it.
    // `.take()` since the batcher's `GroupMember` merge/dedupe logic has no notion of this
    // field and shouldn't carry it along.
    if let Some(events) = group_member_inner.events.take() {
        if !events.is_empty() {
            let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
            let session_id = db::ensure_open_session(&client, auth.group_id).await?;
            for event in &events {
                let mut stored_event = event.clone();
                if let GameEvent::Kill(kill) = &mut stored_event {
                    let occurred_at = kill.occurred_at.unwrap_or_else(Utc::now);
                    kill.participants = Some(
                        db::nearby_members_for_kill(
                            &client,
                            auth.group_id,
                            occurred_at,
                            kill.world,
                            kill.plane,
                            kill.world_x,
                            kill.world_y,
                            &group_member_inner.name,
                        )
                        .await?,
                    );
                }
                db::insert_activity_event(
                    &client,
                    auth.group_id,
                    session_id,
                    &group_member_inner.name,
                    &stored_event,
                )
                .await?;
                discord::dispatch_event_webhook(
                    db_pool.get_ref().clone(),
                    auth.group_id,
                    group_member_inner.name.clone(),
                    event.clone(),
                );

                // Kills are pushed straight to the group's connected overlays too, so other
                // members' chat can react live - unlike vitals, this is worth a dedicated
                // envelope rather than a value bag, and death is left off the wire since
                // nothing consumes it yet.
                if let GameEvent::Kill(kill) = event {
                    if broadcast_registry.has_subscribers(auth.group_id) {
                        let envelope = WsEnvelope::KillEvent {
                            payload: KillEventPayload {
                                member_name: group_member_inner.name.clone(),
                                npc_name: kill.npc_name.clone(),
                            },
                            ts: Utc::now(),
                        };
                        if let Ok(message) = serde_json::to_string(&envelope) {
                            broadcast_registry.publish(auth.group_id, message);
                        }
                    }
                }
            }
        }
    }

    // NPC dialogue/object-interaction events still ride the heartbeat under their own
    // "interactions"/"object_interactions" keys, but they're no longer feed material - the
    // activity feed is scoped to milestones only. They're still `.take()`n so the batcher's
    // `GroupMember` merge/dedupe logic doesn't have to carry fields it has no notion of.
    group_member_inner.interactions.take();
    group_member_inner.object_interactions.take();

    // Quest/diary/combat-task/collection-log milestones aren't discrete plugin events - they're
    // transitions in the full-snapshot progress columns, so they have to be diffed against the
    // stored row *before* the batcher overwrites it. The batcher can't do this itself: it only
    // does latest-value-wins upserts and never reads the old row.
    //
    // Accepted race: the batcher's write can lag this SELECT by up to its flush interval (~50ms),
    // so two heartbeats landing inside that window could diff against the same stale row and
    // double-fire a milestone. Heartbeat cadence is ~1s+, so this is rare enough not to be worth
    // locking/dedup machinery (same tolerance as the non-idempotent deposit retry noted in
    // `update_batcher.rs`).
    {
        let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
        if let Some(previous) =
            db::get_progress_snapshot(&client, auth.group_id, &group_member_inner.name).await?
        {
            let progress_events = progress_events::diff_progress(&previous, &group_member_inner);
            if !progress_events.is_empty() {
                let session_id = db::ensure_open_session(&client, auth.group_id).await?;
                for event in &progress_events {
                    db::insert_progress_event(
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

    // Notable-drop events (loot crossing the plugin's configured value threshold) broadcast a
    // chat message to every connected group member - including the dropper themselves, unlike
    // `KillEvent` above, since nothing else tells them their own drop was notable - and
    // optionally post to Discord. Never stored, matching `alerts`' ephemeral handling above.
    if let Some(notable_drops) = group_member_inner.notable_drops.take() {
        for drop in notable_drops {
            let message = drop.to_message(&group_member_inner.name);

            if broadcast_registry.has_subscribers(auth.group_id) {
                let envelope = WsEnvelope::DropEvent {
                    payload: DropEventPayload {
                        member_name: group_member_inner.name.clone(),
                        message: message.clone(),
                    },
                    ts: Utc::now(),
                };
                if let Ok(json) = serde_json::to_string(&envelope) {
                    broadcast_registry.publish(auth.group_id, json);
                }
            }

            discord::dispatch_drop_webhook(db_pool.get_ref().clone(), auth.group_id, message);
        }
    }

    // Publish straight to any connected party overlays before handing off to
    // the batched DB writer - the batcher trades latency for write
    // efficiency, but the overlay wants these updates as fast as possible.
    if broadcast_registry.has_subscribers(auth.group_id) {
        // `group_member_inner` only carries the fields this particular heartbeat changed (the
        // plugin's DataState diffing skips unchanged ones), so target/spec/active-prayers are
        // routinely absent even while the member is alive and in combat. Broadcasting it as-is
        // would flash those fields to blank on every heartbeat that doesn't happen to touch
        // them. Merge onto the last persisted row first so the wire payload reflects full known
        // state, same as the `roster_snapshot` sent on connect.
        let previous = db::get_group_member(&client, auth.group_id, &group_member_inner.name).await?;
        let mut merged_for_broadcast = previous.unwrap_or_else(|| group_member_inner.clone());
        update_batcher::merge_group_member(&mut merged_for_broadcast, &group_member_inner);

        let envelope = WsEnvelope::VitalsUpdate {
            payload: VitalsUpdatePayload {
                name: group_member_inner.name.clone(),
                vitals: websocket::to_wire_vitals(&merged_for_broadcast),
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
pub async fn get_sessions(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetSessionsQuery>,
) -> Result<web::Json<Vec<GroupSession>>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let sessions = db::list_sessions(&client, auth.group_id, query.limit).await?;
    Ok(web::Json(sessions))
}

#[derive(serde::Serialize)]
pub struct LootBoss {
    pub slug: String,
    pub name: String,
    /// "kill" | "chest" - lets the site group/icon bosses separately from chest sources.
    pub source_type: &'static str,
}

pub async fn get_loot_bosses() -> web::Json<Vec<LootBoss>> {
    let bosses = crate::notable_npcs::names()
        .into_iter()
        .map(|(slug, name)| LootBoss {
            slug,
            name,
            source_type: "kill",
        });
    let chests = crate::loot_sources::names()
        .into_iter()
        .map(|(slug, name)| LootBoss {
            slug,
            name,
            source_type: "chest",
        });
    web::Json(bosses.chain(chests).collect())
}

fn default_loot_sort() -> String {
    "value".to_string()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetLootSummaryQuery {
    #[serde(default)]
    pub member_name: Option<String>,
    #[serde(default)]
    pub session_id: Option<i64>,
    #[serde(default)]
    pub boss: Option<String>,
    #[serde(default)]
    pub clue_tier: Option<String>,
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
    #[serde(default)]
    pub until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub split_mode: Option<String>,
    #[serde(default = "default_loot_sort")]
    pub sort: String,
}
/// One normalized (source_name, source_type, clue_tier, loot, participants) view over either a
/// `GameEvent::Kill` or `GameEvent::Loot` payload, so the summary/split endpoints below don't
/// need parallel match arms for every field they read.
struct LootSourceEvent {
    source_name: String,
    source_type: &'static str,
    clue_tier: Option<String>,
    loot: Vec<LootItem>,
    participants: Option<Vec<String>>,
}
fn as_loot_source_event(event: &GameEvent) -> Option<LootSourceEvent> {
    match event {
        GameEvent::Kill(kill) => Some(LootSourceEvent {
            source_name: kill.npc_name.clone(),
            source_type: "kill",
            clue_tier: None,
            loot: kill.loot.clone().unwrap_or_default(),
            participants: kill.participants.clone(),
        }),
        GameEvent::Loot(loot_event) => Some(LootSourceEvent {
            source_name: loot_event.source_name.clone(),
            source_type: match loot_event.source_type {
                crate::models::LootSourceType::Chest => "chest",
                crate::models::LootSourceType::Clue => "clue",
            },
            clue_tier: loot_event.clue_tier.clone(),
            loot: loot_event.loot.clone(),
            participants: None,
        }),
        GameEvent::Death(_) => None,
    }
}
/// Query-time pivot over `kill`/`loot` activity events into per-(member, source, item) rows,
/// joining GE value (in-process price cache, no live wiki refetch) and rarity/uniqueness
/// (curated `drop_rates` table, skipped for clue sources - see `LootSourceEvent`) at read time -
/// mirrors `groupscape-old`'s `GET /loot-summary`, adapted to this server's flat source name (no
/// `contentKey` slug exists here).
pub async fn get_loot_summary(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetLootSummaryQuery>,
) -> Result<web::Json<Vec<LootSummaryRow>>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let events = db::list_loot_and_kill_events(
        &client,
        auth.group_id,
        query.member_name.as_deref(),
        query.session_id,
        query.since,
        query.until,
    )
    .await?;
    let ge_prices = get_ge_prices_map();

    let mut rows: HashMap<(String, String, i32), LootSummaryRow> = HashMap::new();
    for event in &events {
        let Ok(parsed) = serde_json::from_value::<GameEvent>(event.payload.clone()) else {
            continue;
        };
        let Some(source) = as_loot_source_event(&parsed) else {
            continue;
        };
        if let Some(boss) = query.boss.as_deref() {
            if drop_rates::slugify_npc_name(&source.source_name) != boss {
                continue;
            }
        }
        if let Some(tier) = query.clue_tier.as_deref() {
            if source.clue_tier.as_deref() != Some(tier) {
                continue;
            }
        }
        for item in &source.loot {
            let participants = source
                .participants
                .clone()
                .unwrap_or_else(|| vec![event.member_name.clone()]);
            let participant_count = participants.len().max(1) as i64;
            let names: Vec<String> = match query.split_mode.as_deref() {
                Some("proximity") | Some("personal") => participants.clone(),
                _ => vec![event.member_name.clone()],
            };
            for member_name in names {
                let key = (member_name.clone(), source.source_name.clone(), item.item_id);
                let unit_value = ge_prices.get(&item.item_id).copied();
                let drop = (source.clue_tier.is_none())
                    .then(|| drop_rates::lookup(&source.source_name, item.item_id))
                    .flatten();
                let row = rows.entry(key).or_insert_with(|| LootSummaryRow {
                    member_name,
                    source_name: source.source_name.clone(),
                    source_type: source.source_type.to_string(),
                    clue_tier: source.clue_tier.clone(),
                    item_id: item.item_id,
                    item_name: drop.map(|d| d.name.clone()),
                    quantity: 0,
                    unit_value,
                    total_value: 0,
                    rarity: drop.map(|d| d.rarity.clone()),
                    is_unique: drop.map(|d| d.is_unique).unwrap_or(false),
                    drop_rate: drop.and_then(|d| d.rate.clone()),
                });
                row.quantity += item.quantity;
                let item_value = unit_value.unwrap_or(0) * item.quantity as i64;
                row.total_value += if query.split_mode.as_deref() == Some("proximity") {
                    item_value / participant_count
                } else {
                    item_value
                };
            }
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
    pub member_name: Option<String>,
    #[serde(default)]
    pub session_id: Option<i64>,
    #[serde(default)]
    pub boss: Option<String>,
    #[serde(default)]
    pub clue_tier: Option<String>,
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
    #[serde(default)]
    pub until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub split_mode: Option<String>,
}
/// Total loot GP over `[since, until]` split evenly across every member who reported a kill,
/// chest, or clue event in that range. Unlike `groupscape-old`'s split (which credits a per-kill
/// `actor_character_ids` list), this server has no multi-actor kill co-attribution, so
/// "participants" here is the set of reporting members rather than a raid roster.
pub async fn get_loot_split(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetLootSplitQuery>,
) -> Result<web::Json<LootSplitResult>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let events = db::list_loot_and_kill_events(
        &client,
        auth.group_id,
        query.member_name.as_deref(),
        query.session_id,
        query.since,
        query.until,
    )
    .await?;
    let ge_prices = get_ge_prices_map();

    let mut per_member: HashMap<String, (i64, i64)> = HashMap::new();
    let mut total_value: i64 = 0;
    let mut event_count: i64 = 0;
    for event in &events {
        let Ok(parsed) = serde_json::from_value::<GameEvent>(event.payload.clone()) else {
            continue;
        };
        let Some(source) = as_loot_source_event(&parsed) else {
            continue;
        };
        if let Some(boss) = query.boss.as_deref() {
            if drop_rates::slugify_npc_name(&source.source_name) != boss {
                continue;
            }
        }
        if let Some(tier) = query.clue_tier.as_deref() {
            if source.clue_tier.as_deref() != Some(tier) {
                continue;
            }
        }
        event_count += 1;
        let participants = source
            .participants
            .unwrap_or_else(|| vec![event.member_name.clone()]);
        let names: Vec<String> = match query.split_mode.as_deref() {
            Some("proximity") | Some("personal") => participants.clone(),
            _ => vec![event.member_name.clone()],
        };
        let participant_count = participants.len().max(1) as i64;
        for member_name in names {
            let entry = per_member.entry(member_name).or_insert((0, 0));
            entry.0 += 1;
            for item in &source.loot {
                let value =
                    ge_prices.get(&item.item_id).copied().unwrap_or(0) * item.quantity as i64;
                entry.1 += if query.split_mode.as_deref() == Some("proximity") {
                    value / participant_count
                } else {
                    value
                };
                total_value += value;
            }
        }
    }

    let mut participants: Vec<LootSplitParticipant> = per_member
        .into_iter()
        .map(|(member_name, (events, loot_value))| LootSplitParticipant {
            member_name,
            event_count: events,
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
        event_count,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetLeaderboardQuery {
    pub metric: LeaderboardMetric,
    pub window: LeaderboardWindow,
    #[serde(default)]
    pub boss: Option<String>,
    /// Only meaningful when `metric == Xp`. A `SkillName` string exactly as the client sends it
    /// (e.g. `"Woodcutting"`, `"Overall"`), or omitted for "Overall". See
    /// `db::skill_array_index`'s doc comment for how this is resolved.
    #[serde(default)]
    pub skill: Option<String>,
}
/// XP/boss-KC/GP-earned/loot-value rankings over a daily/weekly/all-time window - part of the
/// Graphs tab rather than a standalone leaderboards page/feature (see [`crate::leaderboard`]).
pub async fn get_leaderboard(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetLeaderboardQuery>,
) -> Result<web::Json<LeaderboardResult>, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let now = Utc::now();

    let (entries, available_bosses) = match query.metric {
        LeaderboardMetric::Xp => {
            let raw = db::get_xp_leaderboard(
                &client,
                auth.group_id,
                query.window,
                query.skill.as_deref(),
                now,
            )
            .await?;
            (crate::leaderboard::rank_entries(raw), vec![])
        }
        LeaderboardMetric::BossKc => {
            let (raw, bosses) = db::get_boss_kc_leaderboard(
                &client,
                auth.group_id,
                query.window,
                query.boss.as_deref(),
                now,
            )
            .await?;
            (crate::leaderboard::rank_entries(raw), bosses)
        }
        LeaderboardMetric::GpEarned => {
            let ge_prices = get_ge_prices_map();
            let raw = db::get_gp_earned_leaderboard(
                &client,
                auth.group_id,
                query.window,
                now,
                &ge_prices,
            )
            .await?;
            (crate::leaderboard::rank_entries(raw), vec![])
        }
        LeaderboardMetric::LootValue => {
            let ge_prices = get_ge_prices_map();
            let raw = db::get_loot_value_leaderboard(
                &client,
                auth.group_id,
                query.window,
                now,
                &ge_prices,
            )
            .await?;
            (crate::leaderboard::rank_entries(raw), vec![])
        }
    };

    Ok(web::Json(LeaderboardResult {
        metric: query.metric,
        window: query.window,
        boss: query.boss.clone(),
        available_bosses,
        entries,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetMetricDataQuery {
    pub metric: LeaderboardMetric,
    pub period: SkillDataPeriod,
    /// Only used when `metric == BossKc`. `None` means "All Bosses" (every kill counted), same
    /// as `get_boss_kc_leaderboard`'s existing `boss: None` semantics.
    #[serde(default)]
    pub boss: Option<String>,
}
/// Graphs tab chart data for boss-KC/GP-earned/loot-value, at the period's bucket granularity,
/// as running cumulative totals (not pre-diffed - the client turns a cumulative series into
/// "gained since period start" itself, mirroring how it already does that for skill XP). XP has
/// its own richer endpoint (`get_skill_data`) and is rejected here.
pub async fn get_metric_data(
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetMetricDataQuery>,
) -> Result<web::Json<GroupMetricData>, Error> {
    if query.metric == LeaderboardMetric::Xp {
        return Err(ApiError::MetricDataXpNotSupportedError.into());
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let aggregate_period = match query.period {
        SkillDataPeriod::Day => db::AggregatePeriod::Day,
        SkillDataPeriod::Week => db::AggregatePeriod::Month,
        SkillDataPeriod::Month => db::AggregatePeriod::Month,
        SkillDataPeriod::Year => db::AggregatePeriod::Year,
    };

    let data = match query.metric {
        LeaderboardMetric::Xp => unreachable!("rejected above"),
        LeaderboardMetric::BossKc => {
            db::get_boss_kc_metric_data(
                &client,
                auth.group_id,
                aggregate_period,
                query.boss.as_deref(),
            )
            .await?
        }
        LeaderboardMetric::GpEarned => {
            db::get_bank_value_for_period(&client, auth.group_id, aggregate_period).await?
        }
        LeaderboardMetric::LootValue => {
            let ge_prices = get_ge_prices_map();
            db::get_loot_value_metric_data(&client, auth.group_id, aggregate_period, &ge_prices)
                .await?
        }
    };

    Ok(web::Json(data))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetItemBonusesQuery {
    pub item_id: i32,
}
/// Equipment bonuses for a single item id (equip-screen Attack/Defence/Other-bonuses columns) -
/// global reference data, not group-scoped, but still requires the same group/character auth as
/// every other endpoint (see main.rs's registration under both scopes). `Authenticated` is
/// required purely as an auth gate here; `auth.group_id` is unused.
pub async fn get_item_bonuses(
    _auth: Authenticated,
    db_pool: web::Data<Pool>,
    query: web::Query<GetItemBonusesQuery>,
) -> Result<web::Json<ItemBonusesResponse>, Error> {
    let bonuses = item_bonuses::get_item_bonuses(db_pool.get_ref(), query.item_id).await?;
    Ok(web::Json(bonuses))
}

pub async fn am_i_logged_in(_auth: Authenticated) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().finish())
}

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

/// Character-scoped counterpart to `am_i_in_group`, mounted on `/api/characters/{account_hash}`
/// instead of `/api/group/{group_name}`. The group-token version above has to check `members`
/// for the *named* row because any client holding the group token can ask about any member
/// name - that's the whole point of the check there. Here, `grouped_character_middleware`
/// already 403s unless this character is linked to a group, so reaching this handler at all is
/// the answer. Checking `members` again would recreate the same bootstrap deadlock the plugin
/// used to hit: a character's `members` row is only ever created as a side effect of its first
/// successful `update-group-member` call, but the plugin gates that very first call behind this
/// endpoint returning success - so a freshly linked character could never pass the check that
/// was supposed to let it start sending the update that creates the row.
pub async fn am_i_in_group_for_character(_auth: Authenticated) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().finish())
}

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
    crate::demo::reject_if_demo(auth.group_id)?;
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

/// Mounted under `character_identity_scope` (no group required) - the plugin calls this once
/// per reconcile cycle regardless of confirm/group status, so a pending character's real RSN
/// reaches the site's confirm card instead of the account_hash placeholder it was created with.
pub async fn identify_character(
    identity: CharacterIdentified,
    body: web::Json<IdentifyCharacter>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let rsn = body.rsn.trim().to_string();
    if !valid_name(&rsn) {
        return Ok(HttpResponse::BadRequest().body("Provided RSN is not valid"));
    }
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::update_character_display_rsn(
        &client,
        identity.character_id,
        &rsn,
        body.combat_level,
        body.total_level,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

/// Mounted under `character_identity_scope` (no group required) - portrait meshes are keyed by
/// `character_id` (`character_mesh`), not `(group_id, member_name)`, so a pending character can
/// have a live 3D render on its confirm card before it ever joins a group.
// Not decorated with #[post(...)] - registered manually in main.rs with its own larger
// PayloadConfig since the global 100KB cap rejects real meshes.
pub async fn update_character_portrait(
    identity: CharacterIdentified,
    // {account_hash} is consumed by the middleware; {member_name} is unused (kept only so the
    // plugin's URL shape mirrors the old `/update-portrait/{member_name}` route).
    _path: web::Path<(String, String)>,
    body: web::Bytes,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::upsert_character_mesh(&client, identity.character_id, &body).await?;
    Ok(HttpResponse::Ok().finish())
}
