use crate::auth_middleware::Authenticated;
use crate::db;
use crate::error::ApiError;
use crate::models::{GroupMember, SHARED_MEMBER};
use actix_web::{rt, web, Error, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use futures_util::StreamExt;
use serde::Serialize;
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
    pub last_heartbeat_at: DateTime<Utc>,
    pub target_name: Option<String>,
    pub target_health_ratio: Option<i32>,
    pub target_health_scale: Option<i32>,
    pub active_prayers: Vec<String>,
    pub rich_presence: Option<String>,
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
        last_heartbeat_at: member.last_updated.unwrap_or_else(Utc::now),
        target_name,
        target_health_ratio,
        target_health_scale,
        active_prayers: member.active_prayers.clone().unwrap_or_default(),
        rich_presence: member.rich_presence.clone(),
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

    rt::spawn(async move {
        if session.text(snapshot_json).await.is_err() {
            return;
        }

        loop {
            tokio::select! {
                incoming = msg_stream.next() => {
                    match incoming {
                        Some(Ok(actix_ws::Message::Ping(bytes))) => {
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(actix_ws::Message::Close(_))) | None => break,
                        Some(Err(_)) => break,
                        _ => {}
                    }
                }
                update = receiver.recv() => {
                    match update {
                        Ok(message) => {
                            if session.text(message).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }

        let _ = session.close(None).await;
    });

    Ok(response)
}
