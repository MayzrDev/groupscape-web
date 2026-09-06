use crate::authed::activity_log_version_key;
use crate::cache::RedisCache;
use crate::db;
use crate::discord;
use crate::error::ApiError;
use crate::models::{RaidCompletionEvent, RaidType};
use crate::unauthed::get_ge_prices_map;
use crate::websocket::{GroupBroadcastRegistry, RaidEventPayload, WsEnvelope};
use actix_web::web::Data;
use chrono::Utc;
use deadpool_postgres::{Client, Pool};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::Mutex;

/// How long a raid completion stays open to more reporters after the first one arrives, before
/// its one websocket/Discord relay fires with whoever showed up. Chosen as a "generous enough for
/// the rest of a full party's clients to report, short enough the feed doesn't lag noticeably" -
/// see the design discussion this shipped from.
pub const MERGE_WINDOW_SECS: u64 = 300;

/// Identifies "the same raid instance" for merging: same group session, same raid, same
/// difficulty (level/mode - see `RaidDifficulty::merge_key`). Deliberately session-wide rather
/// than scoped to confirmed raid-party membership - it's rare for two different sub-parties of
/// the same group session to run the same raid at the same difficulty simultaneously, and
/// detecting real party membership would need new plugin-side party-widget parsing this feature
/// doesn't otherwise require.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RaidMergeKey {
    session_id: i64,
    raid_type: RaidType,
    difficulty_key: String,
}

impl RaidMergeKey {
    pub fn new(session_id: i64, event: &RaidCompletionEvent) -> Self {
        RaidMergeKey {
            session_id,
            raid_type: event.raid_type,
            difficulty_key: event.difficulty.merge_key(),
        }
    }
}

/// Tracks raid completions currently inside their merge window, mapping a [`RaidMergeKey`] to the
/// `activity_events.event_id` a later reporter should append to instead of inserting a new row.
///
/// In-process only, not persisted: a server restart mid-window drops this map, so a reporter
/// arriving after a restart inserts its own separate row rather than merging. Accepted race, same
/// class as this server's other in-memory-only correlation state (e.g. `update_batcher`'s
/// non-idempotent retry window).
pub struct RaidMergeRegistry {
    pub(crate) pending: Mutex<HashMap<RaidMergeKey, i64>>,
}

impl RaidMergeRegistry {
    pub fn new() -> Self {
        RaidMergeRegistry {
            pending: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for RaidMergeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry point called from `authed::update_group_member` for each `GameEvent::Raid` in a
/// heartbeat's events array. Inserts the first report for a raid+difficulty immediately (so the
/// feed shows it in close to real time), or appends to an already-open one; either way, only the
/// *first* reporter's call spawns the delayed finalize task that eventually sends the one
/// websocket/Discord relay for this completion, once [`MERGE_WINDOW_SECS`] has passed and every
/// other reporter within that window has had a chance to append.
#[allow(clippy::too_many_arguments)]
pub async fn handle_raid_completion(
    client: &Client,
    db_pool: Data<Pool>,
    redis: RedisCache,
    registry: Data<RaidMergeRegistry>,
    broadcast_registry: Data<GroupBroadcastRegistry>,
    group_id: i64,
    session_id: i64,
    member_name: String,
    event: RaidCompletionEvent,
) -> Result<(), ApiError> {
    let ge_prices = get_ge_prices_map();
    let key = RaidMergeKey::new(session_id, &event);

    let mut pending = registry.pending.lock().await;
    if let Some(&event_id) = pending.get(&key) {
        drop(pending);
        db::append_raid_participant(client, event_id, &member_name, &event, &ge_prices).await?;
        redis.incr(&activity_log_version_key(group_id)).await;
        return Ok(());
    }

    let event_id =
        db::insert_raid_completion(client, group_id, session_id, &member_name, &event, &ge_prices)
            .await?;
    redis.incr(&activity_log_version_key(group_id)).await;
    pending.insert(key.clone(), event_id);
    drop(pending);

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(MERGE_WINDOW_SECS)).await;
        registry.pending.lock().await.remove(&key);

        let finalize_client = match db_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::warn!("raid_merge: failed to get db client to finalize raid: {}", err);
                return;
            }
        };
        let finalized = match db::finalize_raid_completion(&finalize_client, event_id).await {
            Ok(Some(payload)) => payload,
            Ok(None) => return,
            Err(err) => {
                log::warn!("raid_merge: failed to finalize raid completion: {:?}", err);
                return;
            }
        };
        let previous_best = db::group_raid_best_value(
            &finalize_client,
            group_id,
            finalized.raid_type,
            &finalized.difficulty,
            event_id,
        )
        .await
        .unwrap_or_else(|err| {
            log::warn!("raid_merge: failed to load previous best raid value: {}", err);
            None
        });
        drop(finalize_client);
        redis.incr(&activity_log_version_key(group_id)).await;

        let message = finalized.to_message();
        if broadcast_registry.has_subscribers(group_id) {
            let envelope = WsEnvelope::RaidEvent {
                payload: RaidEventPayload {
                    message: message.clone(),
                },
                ts: Utc::now(),
            };
            if let Ok(json) = serde_json::to_string(&envelope) {
                broadcast_registry.publish(group_id, json);
            }
        }

        discord::dispatch_raid_webhook(db_pool.get_ref().clone(), group_id, finalized, previous_best);
    });

    Ok(())
}
