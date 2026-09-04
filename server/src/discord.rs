use crate::config::DiscordConfig;
use crate::db;
use crate::drop_rates;
use crate::error::ApiError;
use crate::item_names;
use crate::models::{format_gp, GameEvent, GEPrices};
use crate::notable_npcs;
use crate::unauthed;
use deadpool_postgres::Pool;
use serde::Deserialize;
use tokio::task;

const AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";
const TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const USER_URL: &str = "https://discord.com/api/users/@me";

pub struct DiscordUser {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
}

#[derive(Deserialize)]
struct UserResponse {
    id: Option<String>,
    username: Option<String>,
    global_name: Option<String>,
}

/// `identify` scope only - just enough for a stable Discord user id, matching
/// `groupscape-old`'s `discordOAuthClient.ts` decision (ported here, not the raw code, since
/// this stack uses blocking `ureq` rather than `fetch`).
pub fn authorize_url(config: &DiscordConfig, state: &str) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope=identify&state={}",
        AUTHORIZE_URL,
        urlencoding::encode(&config.client_id),
        urlencoding::encode(&config.redirect_uri),
        urlencoding::encode(state),
    )
}

pub async fn exchange_code(config: &DiscordConfig, code: &str) -> Result<DiscordUser, ApiError> {
    let client_id = config.client_id.clone();
    let client_secret = config.client_secret.clone();
    let redirect_uri = config.redirect_uri.clone();
    let code = code.to_owned();

    task::spawn_blocking(move || {
        let token_res = ureq::post(TOKEN_URL)
            .send_form([
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
            ])
            .map_err(ApiError::UreqError)?
            .body_mut()
            .read_json::<TokenResponse>()
            .map_err(ApiError::UreqError)?;

        let Some(access_token) = token_res.access_token else {
            return Err(ApiError::DiscordOAuthError(
                "token exchange returned no access_token".to_string(),
            ));
        };

        let user_res = ureq::get(USER_URL)
            .header("Authorization", &format!("Bearer {}", access_token))
            .call()
            .map_err(ApiError::UreqError)?
            .body_mut()
            .read_json::<UserResponse>()
            .map_err(ApiError::UreqError)?;

        match (user_res.id, user_res.global_name.or(user_res.username)) {
            (Some(id), Some(name)) => Ok(DiscordUser { id, name }),
            _ => Err(ApiError::DiscordOAuthError(
                "user fetch returned incomplete data".to_string(),
            )),
        }
    })
    .await
    .unwrap()
}

const KILL_COLOR: u32 = 0x2ECC71;
const DEATH_COLOR: u32 = 0xE74C3C;
const LOOT_COLOR: u32 = 0xF1C40F;
const DROP_COLOR: u32 = 0x9B59B6;
const RAID_COLOR: u32 = 0xE67E22;
const TEST_COLOR: u32 = 0x5865F2;
const COMBAT_TASK_COLOR: u32 = 0xC084FC;
const COLLECTION_LOG_COLOR: u32 = 0x6C8CFF;
const QUEST_COLOR: u32 = 0xC9962B;
const DIARY_COLOR: u32 = 0x29C2B0;

/// `{quantity}x {name} ({value}, {drop rate})` lines for the unified "Drops" notification -
/// shared by kill loot, chest/clue loot, since both format the same way once resolved to a
/// display line, a gp value, and (when curated) a drop rate.
///
/// The stack's *combined* value gates `min_value` and is what's shown (e.g. 33x a 100gp item is
/// "3,300 gp", not "100 gp") - a single expensive-enough item shouldn't get hidden just because
/// its unit price is small. Untradeable items (no GE price at all) can't be judged against
/// `min_value`, so they always pass the filter and show "untradeable" instead of a gp amount.
fn drop_lines(items: &[crate::models::LootItem], ge_prices: &GEPrices, min_value: i64, source_name: &str) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| {
            let unit_value = ge_prices.get(&item.item_id).copied();
            let value = unit_value.unwrap_or(0) * item.quantity as i64;
            if unit_value.is_some() && value < min_value {
                return None;
            }
            let value_part = match unit_value {
                Some(_) => format!("{} gp", format_gp(value)),
                None => "untradeable".to_string(),
            };
            let detail = match drop_rates::lookup(source_name, item.item_id).and_then(|d| d.rate.clone()) {
                Some(rate) => format!("{}, {}", value_part, rate),
                None => value_part,
            };
            Some(format!("{}x {} ({})", item.quantity, item_names::display(item.item_id), detail))
        })
        .collect()
}

/// Combined GE value of a whole kill's loot, regardless of the per-item `drops_min_value`
/// threshold `drop_lines` applies - the kill message reports what the boss actually dropped, not
/// just the slice a group cares to see itemized in the separate Drops notification.
fn total_loot_value(items: &[crate::models::LootItem], ge_prices: &GEPrices) -> i64 {
    items
        .iter()
        .map(|item| ge_prices.get(&item.item_id).copied().unwrap_or(0) * item.quantity as i64)
        .sum()
}

fn send_webhook_embed_sync(
    url: &str,
    title: &str,
    description: &str,
    color: u32,
    thumbnail_url: Option<&str>,
) -> Result<(), ureq::Error> {
    let mut embed = serde_json::json!({ "title": title, "description": description, "color": color });
    if let Some(thumbnail_url) = thumbnail_url {
        embed["thumbnail"] = serde_json::json!({ "url": thumbnail_url });
    }
    ureq::post(url).send_json(serde_json::json!({ "embeds": [embed] }))?;
    Ok(())
}

/// Awaited (not fire-and-forget) so a bad URL fails the settings save immediately, matching
/// `groupscape-old`'s `discordSettings.ts` validate-before-save behavior.
pub async fn test_webhook(url: &str) -> Result<(), ApiError> {
    let url = url.to_owned();
    task::spawn_blocking(move || {
        send_webhook_embed_sync(
            &url,
            "GroupScape",
            "This channel is now connected to a GroupScape group.",
            TEST_COLOR,
            None,
        )
        .map_err(|err| ApiError::DiscordWebhookInvalidError(err.to_string()))
    })
    .await
    .unwrap_or_else(|err| Err(ApiError::DiscordWebhookInvalidError(err.to_string())))
}

async fn send_webhook_embed(url: String, title: &'static str, description: String, color: u32) {
    send_webhook_embed_with_thumbnail(url, title, description, color, None).await;
}

async fn send_webhook_embed_with_thumbnail(
    url: String,
    title: &'static str,
    description: String,
    color: u32,
    thumbnail_url: Option<String>,
) {
    let _ = task::spawn_blocking(move || {
        if let Err(err) = send_webhook_embed_sync(&url, title, &description, color, thumbnail_url.as_deref()) {
            log::warn!("discord: webhook send failed: {}", err);
        }
    })
    .await;
}

/// Absolute URL to a boss's self-hosted RuneLite-hiscore-style icon (see `site/src/data/
/// boss-icons.js` / `site/public/icons/hiscore/bosses/`), for the "Kill" embed's thumbnail.
/// Not every boss has a downloaded icon in that set - a miss just means Discord's embed proxy
/// fails to fetch the thumbnail and silently omits it, so this doesn't need to check existence.
fn boss_icon_url(web_origin: &str, npc_name: &str) -> String {
    format!(
        "{}/icons/hiscore/bosses/{}.png",
        web_origin.trim_end_matches('/'),
        drop_rates::slugify_npc_name(npc_name)
    )
}

/// Absolute URL to an item's self-hosted icon (see `site/src/data/item.js`'s `Item.imageUrl`),
/// for the "Drops"/"Drop" embed's thumbnail. Uses the base (non-stacked) icon regardless of
/// quantity - the stack-count sprite variants `Item.imageUrl` picks between exist for in-app
/// legibility at a glance, not worth threading through here for a one-off Discord thumbnail.
fn item_icon_url(web_origin: &str, item_id: i32) -> String {
    format!("{}/icons/items/{}.webp", web_origin.trim_end_matches('/'), item_id)
}

/// Absolute URL to the quest-point difficulty icon matching a quest's difficulty, for the
/// "Quest" embed's thumbnail - mirrors `Quest.icon` in `site/src/data/quest.js`. Quests don't
/// have individual icons in this app, only one per difficulty tier.
fn quest_icon_url(web_origin: &str, difficulty: Option<&str>) -> Option<String> {
    let file = match difficulty {
        Some("Novice") => "3399-0.png",
        Some("Intermediate") => "3400-0.png",
        Some("Experienced") => "3402-0.png",
        Some("Master") => "3403-0.png",
        Some("Grandmaster") | Some("Special") => "3404-0.png",
        _ => return None,
    };
    Some(format!("{}/icons/{}", web_origin.trim_end_matches('/'), file))
}

/// Fire-and-forget, mirroring `push::dispatch_alert_push` - a dead or misconfigured webhook must
/// never fail the telemetry upload that triggered it. Loads this group's webhook settings fresh
/// on every call rather than threading them through from the caller, since `update_group_member`
/// may fire this for several events per upload and settings rarely change.
pub fn dispatch_event_webhook(
    db_pool: Pool,
    group_id: i64,
    member_name: String,
    event: GameEvent,
    web_origin: String,
) {
    tokio::spawn(async move {
        let client = match db_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::warn!("discord: failed to get db client: {}", err);
                return;
            }
        };
        let settings = match db::get_discord_webhook_settings(&client, group_id).await {
            Ok(settings) => settings,
            Err(err) => {
                log::warn!("discord: failed to load webhook settings: {}", err);
                return;
            }
        };
        let Some(webhook_url) = settings.webhook_url else {
            return;
        };

        match event {
            GameEvent::Kill(kill) => {
                // Regular NPCs (guards, random monsters) aren't posted - only bosses, matching
                // what a group actually wants pinged in Discord. Uses the same curated boss list
                // as the activity feed (`notable_npcs`), not `drop_rates::is_boss` - that one only
                // covers bosses with a curated drop table, which is a much smaller set.
                if settings.notify_kills && notable_npcs::is_notable(&kill.npc_name) {
                    // Prefer the account's real in-game KC (parsed client-side from the "kill
                    // count is" chat line); only fall back to counting this server's own kill
                    // log when that line didn't arrive in time, e.g. right at plugin startup.
                    let kc = match kill.account_kc {
                        Some(kc) => kc as i64,
                        None => db::count_kills_for_member_npc(&client, group_id, &member_name, &kill.npc_name)
                            .await
                            .unwrap_or_else(|err| {
                                log::warn!("discord: failed to load kill count: {}", err);
                                0
                            }),
                    };
                    let mut description = format!("{} killed {} (KC: {})", member_name, kill.npc_name, kc);
                    if let Some(loot) = &kill.loot {
                        let value = total_loot_value(loot, &unauthed::get_ge_prices_map());
                        if value > 0 {
                            description = format!("{} — loot worth {} gp", description, format_gp(value));
                        }
                    }
                    let thumbnail = boss_icon_url(&web_origin, &kill.npc_name);
                    send_webhook_embed_with_thumbnail(webhook_url.clone(), "Kill", description, KILL_COLOR, Some(thumbnail))
                        .await;
                }
                if settings.notify_drops {
                    if let Some(loot) = &kill.loot {
                        let ge_prices = unauthed::get_ge_prices_map();
                        let lines = drop_lines(loot, &ge_prices, settings.drops_min_value, &kill.npc_name);
                        if !lines.is_empty() {
                            let description = format!(
                                "{} received {} from {}",
                                member_name,
                                lines.join(", "),
                                kill.npc_name
                            );
                            // Thumbnail highlights the single most valuable priced item in the
                            // drop - mirroring the plugin's own notable-drop "highlight" pick -
                            // rather than trying to show every item in one embed image.
                            let thumbnail = loot
                                .iter()
                                .max_by_key(|item| ge_prices.get(&item.item_id).copied().unwrap_or(0) * item.quantity as i64)
                                .map(|item| item_icon_url(&web_origin, item.item_id));
                            send_webhook_embed_with_thumbnail(webhook_url, "Drops", description, LOOT_COLOR, thumbnail).await;
                        }
                    }
                }
            }
            GameEvent::Death(death) => {
                if settings.notify_deaths {
                    let description = match &death.killer_name {
                        Some(killer) => format!("{} died to {}", member_name, killer),
                        None => format!("{} died", member_name),
                    };
                    send_webhook_embed(webhook_url, "Death", description, DEATH_COLOR).await;
                }
            }
            GameEvent::Loot(loot_event) => {
                if settings.notify_drops && !loot_event.loot.is_empty() {
                    let ge_prices = unauthed::get_ge_prices_map();
                    let lines = drop_lines(&loot_event.loot, &ge_prices, settings.drops_min_value, &loot_event.source_name);
                    if !lines.is_empty() {
                        let description = format!(
                            "{} received {} from {}",
                            member_name,
                            lines.join(", "),
                            loot_event.source_name
                        );
                        let thumbnail = loot_event
                            .loot
                            .iter()
                            .max_by_key(|item| ge_prices.get(&item.item_id).copied().unwrap_or(0) * item.quantity as i64)
                            .map(|item| item_icon_url(&web_origin, item.item_id));
                        send_webhook_embed_with_thumbnail(webhook_url, "Drops", description, LOOT_COLOR, thumbnail).await;
                    }
                }
            }
            // Raid completions aren't relayed per-event like the other variants above - they go
            // through `dispatch_raid_webhook` instead, called once from `raid_merge`'s finalize
            // step after the merge window closes, so a group's whole party doesn't each trigger
            // a separate near-duplicate Discord post for the same raid.
            GameEvent::Raid(_) => {}
        }
    });
}

/// Fire-and-forget, mirroring `dispatch_event_webhook` above - posts the merged raid-completion
/// message (`RaidCompletionPayload::to_message`) as a Discord embed, once per completion, when
/// the group has a webhook configured and `notify_raids` is on. Like `dispatch_drop_webhook`,
/// takes the pre-built message directly since the merge/finalize step that produces it already
/// lives outside this per-event match.
pub fn dispatch_raid_webhook(db_pool: Pool, group_id: i64, message: String) {
    tokio::spawn(async move {
        let client = match db_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::warn!("discord: failed to get db client: {}", err);
                return;
            }
        };
        let settings = match db::get_discord_webhook_settings(&client, group_id).await {
            Ok(settings) => settings,
            Err(err) => {
                log::warn!("discord: failed to load webhook settings: {}", err);
                return;
            }
        };
        drop(client);
        let Some(webhook_url) = settings.webhook_url else {
            return;
        };
        if !settings.notify_raids {
            return;
        }

        send_webhook_embed(webhook_url, "Raid", message, RAID_COLOR).await;
    });
}

/// Fire-and-forget, mirroring `dispatch_event_webhook` above - posts the same message text
/// already built for the roster-websocket broadcast (`NotableDropEvent::to_message`) as a
/// Discord embed, when the group has a webhook configured and `notify_drops` is on. Notable
/// drops aren't a `GameEvent` (they're never stored, see `update_group_member`), so this takes
/// the pre-built message directly rather than matching over the enum like the function above.
pub fn dispatch_drop_webhook(db_pool: Pool, group_id: i64, message: String, item_id: i32, web_origin: String) {
    tokio::spawn(async move {
        let client = match db_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::warn!("discord: failed to get db client: {}", err);
                return;
            }
        };
        let settings = match db::get_discord_webhook_settings(&client, group_id).await {
            Ok(settings) => settings,
            Err(err) => {
                log::warn!("discord: failed to load webhook settings: {}", err);
                return;
            }
        };
        drop(client);
        let Some(webhook_url) = settings.webhook_url else {
            return;
        };
        if !settings.notify_drops {
            return;
        }

        let thumbnail = item_icon_url(&web_origin, item_id);
        send_webhook_embed_with_thumbnail(webhook_url, "Drop", message, DROP_COLOR, Some(thumbnail)).await;
    });
}

/// Fire-and-forget, mirroring `dispatch_event_webhook` above - posts a `ProgressEvent` (quest,
/// diary, combat-achievement, or collection-log milestone) as a Discord embed when the matching
/// notify setting is on.
///
/// Per-task combat-achievement and per-item collection-log events are far too frequent to post
/// individually (hundreds of tasks / thousands of log slots) - only the boss-group and
/// page-completion variants of those two event types (`kind: "boss"` / `kind: "page"` in the
/// payload) are posted, matching the milestone framing the settings row describes.
pub fn dispatch_progress_webhook(
    db_pool: Pool,
    group_id: i64,
    member_name: String,
    event: crate::progress_events::ProgressEvent,
    web_origin: String,
) {
    use crate::progress_events::{EVENT_TYPE_COLLECTION_LOG, EVENT_TYPE_COMBAT_TASK, EVENT_TYPE_DIARY, EVENT_TYPE_QUEST};

    tokio::spawn(async move {
        let client = match db_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::warn!("discord: failed to get db client: {}", err);
                return;
            }
        };
        let settings = match db::get_discord_webhook_settings(&client, group_id).await {
            Ok(settings) => settings,
            Err(err) => {
                log::warn!("discord: failed to load webhook settings: {}", err);
                return;
            }
        };
        drop(client);
        let Some(webhook_url) = settings.webhook_url else {
            return;
        };

        let post = match event.event_type {
            EVENT_TYPE_QUEST if settings.notify_quests => {
                let quest_id = event.payload["quest_id"].as_i64().unwrap_or_default() as i32;
                let name = crate::quest_ids::quest_name(quest_id).unwrap_or("a quest");
                let thumbnail = quest_icon_url(&web_origin, crate::quest_ids::quest_difficulty(quest_id));
                Some((
                    "Quest",
                    format!("{} completed {}", member_name, name),
                    QUEST_COLOR,
                    thumbnail,
                ))
            }
            EVENT_TYPE_DIARY if settings.notify_diaries => {
                let region = event.payload["region"].as_str().unwrap_or("");
                let tier = event.payload["tier"].as_str().unwrap_or("");
                Some((
                    "Diary",
                    format!("{} completed the {} {} diary", member_name, region, tier),
                    DIARY_COLOR,
                    None,
                ))
            }
            EVENT_TYPE_COMBAT_TASK if settings.notify_combat_achievements && event.payload["kind"] == "boss" => {
                let boss = event.payload["boss"].as_str().unwrap_or("a boss");
                Some((
                    "Combat achievements",
                    format!("{} completed every combat achievement for {}", member_name, boss),
                    COMBAT_TASK_COLOR,
                    None,
                ))
            }
            EVENT_TYPE_COLLECTION_LOG if settings.notify_collection_log && event.payload["kind"] == "page" => {
                let page = event.payload["page"].as_str().unwrap_or("a page");
                Some((
                    "Collection log",
                    format!("{} completed the {} collection log page", member_name, page),
                    COLLECTION_LOG_COLOR,
                    None,
                ))
            }
            _ => None,
        };

        if let Some((title, description, color, thumbnail)) = post {
            send_webhook_embed_with_thumbnail(webhook_url, title, description, color, thumbnail).await;
        }
    });
}
