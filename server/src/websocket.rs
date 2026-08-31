use crate::auth_middleware::Authenticated;
use crate::db;
use crate::error::ApiError;
use crate::models::{GroupMember, SHARED_MEMBER};
use actix_web::{rt, web, Error, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 256;

/// Stable per-member display colors assigned by join order, mirroring
/// groupscape-old's JOIN_ORDER_PALETTE.
const JOIN_ORDER_PALETTE: [&str; 8] = [
    "#5B8DEF", "#E4572E", "#4CAF50", "#E8C547", "#9B59B6", "#1ABC9C", "#E67E22", "#EC407A",
];

pub fn member_color(join_order_index: usize) -> String {
    JOIN_ORDER_PALETTE[join_order_index % JOIN_ORDER_PALETTE.len()].to_string()
}

/// Per-group fan-out registry for the party overlay WebSocket. Each connected
/// overlay subscribes to its group's broadcast channel; `update-group-member`
/// publishes a lightweight vitals-only message into it, bypassing the
/// batched DB writer so overlay updates stay low latency.
pub struct GroupBroadcastRegistry {
    inner: RwLock<HashMap<i64, broadcast::Sender<String>>>,
}

impl GroupBroadcastRegistry {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn subscribe(&self, group_id: i64) -> broadcast::Receiver<String> {
        let mut inner = self.inner.write().unwrap();
        let sender = inner
            .entry(group_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0);
        sender.subscribe()
    }

    pub fn has_subscribers(&self, group_id: i64) -> bool {
        let inner = self.inner.read().unwrap();
        inner
            .get(&group_id)
            .map(|sender| sender.receiver_count() > 0)
            .unwrap_or(false)
    }

    pub fn publish(&self, group_id: i64, message: String) {
        let inner = self.inner.read().unwrap();
        if let Some(sender) = inner.get(&group_id) {
            let _ = sender.send(message);
        }
    }
}

impl Default for GroupBroadcastRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Longest a ping can go without a fresh Start/Update before `PingRegistry::list_active` prunes
/// it - a safety net for the web poll path in case a client crashes/disconnects without ever
/// sending `PingAction::End` (the plugin's own 60s client-side timeout is the primary expiry;
/// this is deliberately looser so it never prunes a still-live ping out from under a slightly
/// slow heartbeat).
const PING_TTL: std::time::Duration = std::time::Duration::from_secs(70);

/// One group member's active ping, as tracked in-memory for the web map's poll endpoint
/// (`get_active_pings`). The RuneLite-facing path doesn't need this snapshot at all - it just
/// forwards `PingStart`/`PingUpdate`/`PingEnd` frames straight through `GroupBroadcastRegistry` as
/// they arrive - but the web site has no websocket (see `authed::submit_ping`'s doc comment) and
/// polls instead, which needs something to poll *from*.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ActivePing {
    pub ping_id: String,
    pub member_name: String,
    pub kind: PingKind,
    pub x: i32,
    pub y: i32,
    pub plane: i32,
    pub npc_name: Option<String>,
    #[serde(skip)]
    pub last_seen: std::time::Instant,
}

/// Per-group active-ping table backing the web map's poll endpoint. Never persisted to the DB -
/// same ephemeral treatment as `GroupBroadcastRegistry`'s in-memory channels.
pub struct PingRegistry {
    inner: RwLock<HashMap<i64, HashMap<String, ActivePing>>>,
}

impl PingRegistry {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn start(&self, group_id: i64, ping: ActivePing) {
        let mut inner = self.inner.write().unwrap();
        inner
            .entry(group_id)
            .or_insert_with(HashMap::new)
            .insert(ping.ping_id.clone(), ping);
    }

    pub fn update(&self, group_id: i64, ping_id: &str, x: i32, y: i32, plane: i32) {
        let mut inner = self.inner.write().unwrap();
        if let Some(group) = inner.get_mut(&group_id) {
            if let Some(ping) = group.get_mut(ping_id) {
                ping.x = x;
                ping.y = y;
                ping.plane = plane;
                ping.last_seen = std::time::Instant::now();
            }
        }
    }

    pub fn end(&self, group_id: i64, ping_id: &str) {
        let mut inner = self.inner.write().unwrap();
        if let Some(group) = inner.get_mut(&group_id) {
            group.remove(ping_id);
        }
    }

    /// Prunes anything past `PING_TTL` for this group, then returns what's left.
    pub fn list_active(&self, group_id: i64) -> Vec<ActivePing> {
        let mut inner = self.inner.write().unwrap();
        let Some(group) = inner.get_mut(&group_id) else {
            return Vec::new();
        };
        group.retain(|_, ping| ping.last_seen.elapsed() < PING_TTL);
        group.values().cloned().collect()
    }
}

impl Default for PingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WireVitals {
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    pub prayer: Option<i32>,
    pub max_prayer: Option<i32>,
    pub run_energy: Option<i32>,
    pub spec_energy: Option<i32>,
    pub world: Option<i32>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub target_name: Option<String>,
    pub target_health_ratio: Option<i32>,
    pub target_health_scale: Option<i32>,
    pub active_prayers: Vec<String>,
    pub rich_presence: Option<String>,
    /// `[x, y, plane]` (sometimes `[x, y, plane, is_on_boat, world]` - see `authed.rs`'s
    /// `record_location_sample`), truncated to 3 once persisted to the `members` row but carried
    /// through in full on the broadcast merge. Powers the in-game world map/minimap markers.
    pub coordinates: Option<Vec<i32>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterMemberEntry {
    pub name: String,
    pub color: String,
    pub vitals: Option<WireVitals>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterSnapshotPayload {
    pub roster: Vec<RosterMemberEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VitalsUpdatePayload {
    pub name: String,
    pub vitals: WireVitals,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KillEventPayload {
    pub member_name: String,
    pub npc_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropEventPayload {
    pub member_name: String,
    /// Pre-built by `NotableDropEvent::to_message` - the plugin relays this verbatim.
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorUpdatePayload {
    pub name: String,
    pub color: String,
}

/// What a ping was dropped on - see `PingStartPayload::npc_name`.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PingKind {
    Tile,
    Npc,
}

/// A ping's lifecycle rides its own dedicated envelope variants (Start/Update/End) rather than
/// piggybacking on `WireVitals` - unlike vitals, a ping is a discrete event stream, not continuous
/// per-tick state. `pingId` lets receivers match `PingUpdate`/`PingEnd` frames back to the
/// `PingStart` they belong to (one player can only have one active ping - see
/// `authed::submit_ping` - but a fresh ping's id still needs to be distinguishable from a stale one
/// still in flight over the wire).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PingStartPayload {
    pub ping_id: String,
    pub member_name: String,
    pub kind: PingKind,
    pub x: i32,
    pub y: i32,
    pub plane: i32,
    /// Set only for `PingKind::Npc` - the tracked NPC's name, for the marker tooltip/chat line.
    pub npc_name: Option<String>,
}

/// Live-tracking re-broadcast of an NPC ping's current tile - only the pinging player's own
/// client observes the NPC and resends its position; other clients never resolve the NPC locally.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PingUpdatePayload {
    pub ping_id: String,
    pub x: i32,
    pub y: i32,
    pub plane: i32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PingEndPayload {
    pub ping_id: String,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEnvelope {
    RosterSnapshot {
        payload: RosterSnapshotPayload,
        ts: DateTime<Utc>,
    },
    VitalsUpdate {
        payload: VitalsUpdatePayload,
        ts: DateTime<Utc>,
    },
    KillEvent {
        payload: KillEventPayload,
        ts: DateTime<Utc>,
    },
    DropEvent {
        payload: DropEventPayload,
        ts: DateTime<Utc>,
    },
    ColorUpdate {
        payload: ColorUpdatePayload,
        ts: DateTime<Utc>,
    },
    PingStart {
        payload: PingStartPayload,
        ts: DateTime<Utc>,
    },
    PingUpdate {
        payload: PingUpdatePayload,
        ts: DateTime<Utc>,
    },
    PingEnd {
        payload: PingEndPayload,
        ts: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_event_serializes_with_snake_case_type_and_camel_case_payload() {
        let envelope = WsEnvelope::KillEvent {
            payload: KillEventPayload {
                member_name: "Zezima".to_string(),
                npc_name: "Zulrah".to_string(),
            },
            ts: DateTime::<Utc>::MIN_UTC,
        };

        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["type"], "kill_event");
        assert_eq!(json["payload"]["memberName"], "Zezima");
        assert_eq!(json["payload"]["npcName"], "Zulrah");
    }
}

/// Maps the persisted/POSTed GroupMember shape onto the overlay's wire vitals.
pub fn to_wire_vitals(member: &GroupMember) -> WireVitals {
    let (hp, max_hp, prayer, max_prayer, run_energy, world) = match &member.stats {
        Some(stats) if stats.len() >= 7 => (
            Some(stats[0]),
            Some(stats[1]),
            Some(stats[2]),
            Some(stats[3]),
            Some(stats[4]),
            Some(stats[6]),
        ),
        _ => (None, None, None, None, None, None),
    };

    let (target_name, target_health_ratio, target_health_scale) = match &member.interacting {
        Some(interacting) => (
            Some(interacting.name.clone()),
            Some(interacting.ratio),
            Some(interacting.scale),
        ),
        None => (None, None, None),
    };

    WireVitals {
        hp,
        max_hp,
        prayer,
        max_prayer,
        run_energy,
        spec_energy: member.special_attack,
        world,
        last_heartbeat_at: member.last_updated,
        target_name,
        target_health_ratio,
        target_health_scale,
        active_prayers: member.active_prayers.clone().unwrap_or_default(),
        rich_presence: member.rich_presence.clone(),
        coordinates: member.coordinates.clone(),
    }
}

/// `GET /api/group/{group_name}/ws` - real-time push feed for the RuneLite
/// party overlay. Authenticated identically to the rest of `authed_scope`
/// (same group-token header, verified by `AuthenticateMiddlewareFactory`
/// before this handler runs). On connect, sends one `roster_snapshot` built
/// from the current DB state, then forwards this group's broadcast channel
/// as `vitals_update` frames until the socket closes.
pub async fn party_overlay_ws(
    req: HttpRequest,
    stream: web::Payload,
    auth: Authenticated,
    db_pool: web::Data<Pool>,
    registry: web::Data<GroupBroadcastRegistry>,
) -> Result<HttpResponse, Error> {
    let group_id = auth.group_id;
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let colors = db::get_member_color_map(&client, group_id).await?;
    let members = db::get_group_data(
        &client,
        group_id,
        &DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
    )
    .await?;
    drop(client);

    let roster: Vec<RosterMemberEntry> = members
        .iter()
        .filter(|member| member.name != SHARED_MEMBER)
        .map(|member| RosterMemberEntry {
            name: member.name.clone(),
            // Prefer the admin-assigned helmet colour (`members.color`) over the join-order
            // fallback palette, so the overlay stripe matches the site's colour picker. Falls
            // back to join order for members nobody has assigned a colour to yet.
            color: member
                .color
                .as_deref()
                .and_then(crate::models::named_color_to_hex)
                .or_else(|| colors.get(&member.name).cloned())
                .unwrap_or_else(|| "#808080".to_string()),
            vitals: Some(to_wire_vitals(member)),
        })
        .collect();

    let snapshot = WsEnvelope::RosterSnapshot {
        payload: RosterSnapshotPayload { roster },
        ts: Utc::now(),
    };
    let snapshot_json = serde_json::to_string(&snapshot).map_err(ApiError::SerdeJsonError)?;

    let mut receiver = registry.subscribe(group_id);
    let account_hash = auth.account_hash.clone().unwrap_or_default();
    log::info!(
        "Party overlay WebSocket connected (group_id={}, account_hash={})",
        group_id,
        account_hash
    );

    rt::spawn(async move {
        if session.text(snapshot_json).await.is_err() {
            log::info!(
                "Party overlay WebSocket closed: failed to send initial snapshot (group_id={}, account_hash={})",
                group_id, account_hash
            );
            return;
        }

        let close_reason = loop {
            tokio::select! {
                incoming = msg_stream.next() => {
                    match incoming {
                        Some(Ok(actix_ws::Message::Ping(bytes))) => {
                            if session.pong(&bytes).await.is_err() {
                                break "failed to send pong";
                            }
                        }
                        Some(Ok(actix_ws::Message::Close(_))) => break "client sent close",
                        None => break "client stream ended",
                        Some(Err(_)) => break "client stream error",
                        _ => {}
                    }
                }
                update = receiver.recv() => {
                    match update {
                        Ok(message) => {
                            if session.text(message).await.is_err() {
                                break "failed to send update";
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            log::warn!(
                                "Party overlay WebSocket receiver lagged, dropped {} messages (group_id={}, account_hash={})",
                                skipped, group_id, account_hash
                            );
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => break "broadcast channel closed",
                    }
                }
            }
        };
        log::info!(
            "Party overlay WebSocket disconnected: {} (group_id={}, account_hash={})",
            close_reason, group_id, account_hash
        );

        let _ = session.close(None).await;
    });

    Ok(response)
}
