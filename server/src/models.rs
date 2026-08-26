use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const SHARED_MEMBER: &str = "@SHARED";

/// Helmet/accent colour keys a member row's `color` column may hold. Shared vocabulary with the
/// site's `member-data.js` palette - the hue each key maps to is a frontend-only concern, so
/// only the keys themselves need to agree between the two.
pub const MEMBER_COLOR_PALETTE: [&str; 12] = [
    "yellow", "green", "blue", "red", "purple", "orange", "cyan", "pink", "lime", "teal", "indigo",
    "brown",
];

/// Hue (degrees) each [`MEMBER_COLOR_PALETTE`] key renders as, mirroring the site's
/// `MEMBER_COLOR_HUES` (member-data.js) exactly - `hsl(hue, 70%, 45%)`. Lets server code (the
/// party overlay websocket) hand the RuneLite plugin a decoded hex colour instead of the raw key,
/// so the overlay accent matches the site's colour picker without the plugin needing its own hue
/// table.
const MEMBER_COLOR_HUES: [(&str, f64); 12] = [
    ("yellow", 41.0),
    ("green", 151.0),
    ("blue", 210.0),
    ("red", 355.0),
    ("purple", 288.0),
    ("orange", 25.0),
    ("cyan", 185.0),
    ("pink", 330.0),
    ("lime", 95.0),
    ("teal", 170.0),
    ("indigo", 245.0),
    ("brown", 30.0),
];

/// Resolves a [`MEMBER_COLOR_PALETTE`] key to the hex string its `hsl(hue, 70%, 45%)` rendering
/// works out to. `None` for a key not in [`MEMBER_COLOR_HUES`].
pub fn named_color_to_hex(key: &str) -> Option<String> {
    MEMBER_COLOR_HUES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, hue)| hsl_to_hex(*hue, 0.70, 0.45))
}

fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to_byte = |v: f64| ((v + m) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", to_byte(r1), to_byte(g1), to_byte(b1))
}

#[cfg(test)]
mod member_color_tests {
    use super::*;

    #[test]
    fn named_color_to_hex_matches_known_hsl_conversion() {
        // hsl(151, 70%, 45%) -> a mid-tone green.
        assert_eq!(named_color_to_hex("green").as_deref(), Some("#22C375"));
        assert_eq!(named_color_to_hex("not-a-color"), None);
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Coordinates {
    x: i32,
    y: i32,
    plane: i32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Interacting {
    pub name: String,
    pub scale: i32,
    pub ratio: i32,
    location: Coordinates,
    #[serde(default = "default_last_updated")]
    last_updated: DateTime<Utc>,
}
fn default_last_updated() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CombatAchievements {
    pub tiers: std::collections::HashMap<String, bool>,
    pub tasks: std::collections::HashMap<String, bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupMemberName {
    pub name: String,
}

#[derive(Serialize)]
pub struct BlockedMember {
    pub member_name: String,
    pub blocked_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameGroup {
    pub new_name: String,
}

#[derive(Serialize)]
pub struct GroupCredentials {
    pub name: String,
    pub token: String,
}

#[derive(Deserialize, Serialize)]
pub struct GroupMember {
    #[serde(skip)]
    pub group_id: Option<i64>,
    pub name: String,
    /// Plugin-submitted `client.getAccountHash()`, used to derive `name` server-side from the
    /// linked character's `display_rsn` instead of requiring it to be typed at group setup.
    /// `None` for legacy plugin builds or unlinked characters, which fall back to matching an
    /// existing member row by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_hash: Option<String>,
    /// Admin-assigned helmet/accent colour (one of [`MEMBER_COLOR_PALETTE`]), unconditionally
    /// included rather than gated by the heartbeat's `$1` staleness cutoff like the fields below
    /// - it changes far less often than telemetry, so there's no "since timestamp" to check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quests: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equipment: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_bank: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rune_pouch: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interacting: Option<Interacting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_vault: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposited: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diary_vars: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_log_v2: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub potion_storage: Option<Vec<i32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_attack: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_prayers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_presence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat_achievements: Option<CombatAchievements>,
    /// Timestamp of the character's most recent portrait mesh upload (`character_mesh.mesh_last_update`),
    /// gated by the same "since timestamp" cutoff as the telemetry fields above. Never sent by the
    /// plugin - it's server-computed in `get_group_data` so an already-open side panel knows to
    /// refetch `/portrait/{member_name}` after a gear/appearance change instead of showing a stale mesh
    /// until the page is reloaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait_last_update: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<DateTime<Utc>>,
    /// Discrete kill/death events piggybacked on the same heartbeat, matching
    /// `groupscape-old`'s "ride the tick" ingestion pattern rather than a separate endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<GameEvent>>,
    /// NPC dialogue events from the plugin's `InteractionEvents` accumulator, under its own
    /// "interactions" upload key (kept separate from `events` and `object_interactions` -
    /// the plugin never merges these event streams).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interactions: Option<Vec<DialogueEvent>>,
    /// Object-interaction events from the plugin's `ObjectInteractionEvents` accumulator,
    /// under its own "object_interactions" upload key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_interactions: Option<Vec<ObjectInteractionEvent>>,
    /// Low-HP/wilderness-entry alert events from the plugin's `AlertEvents` accumulator, under
    /// its own "alerts" upload key. Consumed once to trigger a web push, never stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alerts: Option<Vec<AlertEvent>>,
}

/// One item entry in a [`GameEvent::Kill`]'s loot, field names matching the plugin's
/// `PendingKill.toMap()` output verbatim (`itemId`/`quantity`).
#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LootItem {
    pub item_id: i32,
    pub quantity: i32,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KillEvent {
    pub npc_id: i32,
    pub npc_name: String,
    pub world_x: i32,
    pub world_y: i32,
    pub plane: i32,
    pub world: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participants: Option<Vec<String>>,
    /// Absent when the plugin's best-effort loot correlation (`onLoot`) never matched a
    /// pending kill before the next drain - the kill still ships without loot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot: Option<Vec<LootItem>>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DeathEvent {
    pub world_x: i32,
    pub world_y: i32,
    pub plane: i32,
    pub world: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<DateTime<Utc>>,
    /// Best-effort, from `Actor.getInteracting()` at time of death - never a guaranteed
    /// "killed by" fact, so this is `None` rather than defaulted to any placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub killer_name: Option<String>,
}

/// Discriminated on the plugin's own `"type"` field ("kill"/"death"), matching
/// `KillLootDeathEvents`' transport shape field-for-field.
#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum GameEvent {
    Kill(KillEvent),
    Death(DeathEvent),
}
impl GameEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            GameEvent::Kill(_) => "kill",
            GameEvent::Death(_) => "death",
        }
    }
}

/// One NPC dialogue event, field names matching `InteractionEvents.onDialogue`'s transport
/// shape verbatim (`groupscape-plugin`'s "interactions" upload key). The plugin always sends
/// `"type": "dialogue"` even though it's the only shape on this key today - kept as a plain
/// field rather than an enum tag like [`GameEvent`] since there's nothing to discriminate yet.
#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DialogueEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub npc_id: i32,
    pub npc_name: String,
    pub combat_level: i32,
    pub world_x: i32,
    pub world_y: i32,
    pub plane: i32,
    pub world: i32,
}

/// One object-interaction event, field names matching
/// `ObjectInteractionEvents.onObjectInteraction`'s transport shape verbatim
/// (`groupscape-plugin`'s "object_interactions" upload key). Unlike [`DialogueEvent`], the
/// plugin never tags these with a "type" field.
#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ObjectInteractionEvent {
    pub object_id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub world_x: i32,
    pub world_y: i32,
    pub plane: i32,
    pub world: i32,
}

/// Discriminated on the plugin's own `"type"` field ("low_hp"/"wilderness_entry"), matching
/// `AlertEvents`' transport shape field-for-field (`groupscape-plugin`'s "alerts" upload key).
/// Unlike [`GameEvent`], these never land in `activity_events` - they're consumed once, straight
/// off the heartbeat, to trigger a web push and are not stored.
#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertEvent {
    LowHp(LowHpAlert),
    WildernessEntry(WildernessEntryAlert),
}
impl AlertEvent {
    /// Matches the enum's own serde tag ("low_hp"/"wilderness_entry") - used as both the push
    /// payload's `type` field and its notification `tag` (so a fresh low-HP alert replaces a
    /// stale one instead of stacking, per `groupscape-web#39`'s design review).
    pub fn alert_type(&self) -> &'static str {
        match self {
            AlertEvent::LowHp(_) => "low_hp",
            AlertEvent::WildernessEntry(_) => "wilderness_entry",
        }
    }

    /// Low-HP alerts stay on screen until dismissed - unlike the other alert/toast copy in this
    /// app, this one exists specifically for when the player isn't looking at the tab, so letting
    /// it auto-vanish after a few seconds defeats the point. Wilderness entry is informational
    /// and auto-dismisses like a normal OS notification.
    pub fn requires_interaction(&self) -> bool {
        matches!(self, AlertEvent::LowHp(_))
    }

    /// Push notification title/body, mirroring `groupscape-old`'s alert-push copy, with the
    /// member name appended so a multi-character account can tell which one fired.
    pub fn push_title_and_body(&self, member_name: &str) -> (String, String) {
        match self {
            AlertEvent::LowHp(alert) => (
                format!("Low HP — {}", member_name),
                format!("{}/{} HP remaining", alert.current_hp, alert.max_hp),
            ),
            AlertEvent::WildernessEntry(alert) => (
                format!("Wilderness entry — {}", member_name),
                format!("Entered level {} wilderness", alert.wilderness_level),
            ),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LowHpAlert {
    pub current_hp: i32,
    pub max_hp: i32,
    pub world_x: i32,
    pub world_y: i32,
    pub plane: i32,
    pub world: i32,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WildernessEntryAlert {
    pub wilderness_level: i32,
    pub world_x: i32,
    pub world_y: i32,
    pub plane: i32,
    pub world: i32,
}

#[derive(Serialize)]
pub struct ActivityEvent {
    pub id: i64,
    pub session_id: i64,
    pub member_name: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}

/// One aggregated (member, npc, item) row for the loot summary endpoint - a query-time pivot
/// over `kill` activity events, mirroring `groupscape-old`'s `LootSummaryRow`.
#[derive(Serialize)]
pub struct LootSummaryRow {
    pub member_name: String,
    pub npc_name: String,
    pub item_id: i32,
    pub item_name: Option<String>,
    pub quantity: i32,
    pub unit_value: Option<i64>,
    pub total_value: i64,
    pub rarity: Option<String>,
    pub is_unique: bool,
    pub drop_rate: Option<String>,
}

/// Wire shape for `GET .../get-item-bonuses` - the equip-screen bonus panel's Attack/Defence/
/// Other-bonuses columns, scraped from the OSRS Wiki and cached server-side (30-day TTL, see
/// [`crate::item_bonuses`]).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ItemBonusesResponse {
    pub item_id: i32,
    pub attack: CombatStyleBonuses,
    pub defence: CombatStyleBonuses,
    pub melee_strength: i32,
    pub ranged_strength: i32,
    /// Tenths of a percent (30 = 3.0%), so it round-trips exactly without floats.
    pub magic_damage: i32,
    pub prayer: i32,
    /// Game ticks. `None` for non-weapon equipment - the wiki's `{{Infobox Bonuses}}` template
    /// omits `speed` entirely for those.
    pub attack_speed: Option<i32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CombatStyleBonuses {
    pub stab: i32,
    pub slash: i32,
    pub crush: i32,
    pub magic: i32,
    pub ranged: i32,
}

#[derive(Serialize)]
pub struct LootSplitParticipant {
    pub member_name: String,
    pub kill_count: i64,
    pub loot_value: i64,
}

/// This server has no multi-actor kill co-attribution (each kill event belongs to exactly one
/// reporting member), so unlike `groupscape-old`'s split, participants here are every member who
/// reported a kill in the range, not a per-kill actor list.
#[derive(Serialize)]
pub struct LootSplitResult {
    pub total_value: i64,
    pub kill_count: i64,
    pub participants: Vec<LootSplitParticipant>,
    pub per_person_gp: i64,
    pub remainder_gp: i64,
}

#[derive(Serialize)]
pub struct GroupSession {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}
#[derive(Serialize)]
pub struct AggregateSkillData {
    pub time: DateTime<Utc>,
    pub data: Vec<i32>,
}
#[derive(Serialize)]
pub struct MemberSkillData {
    pub name: String,
    pub skill_data: Vec<AggregateSkillData>,
}
pub type GroupSkillData = Vec<MemberSkillData>;
#[derive(Serialize)]
pub struct MetricDataPoint {
    pub time: DateTime<Utc>,
    pub value: i64,
}
#[derive(Serialize)]
pub struct MemberMetricData {
    pub name: String,
    pub metric_data: Vec<MetricDataPoint>,
}
pub type GroupMetricData = Vec<MemberMetricData>;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateGroup {
    pub name: String,
    pub member_names: Vec<String>,
    #[serde(default, skip_serializing)]
    pub captcha_response: String,
    #[serde(default = "default_token")]
    #[serde(skip_deserializing)]
    pub token: String,
}
fn default_token() -> String {
    uuid::Uuid::new_v4().hyphenated().to_string()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmIInGroupRequest {
    pub member_name: String,
}
#[derive(Deserialize)]
pub struct WikiGEPrice {
    pub high: Option<i64>,
    pub low: Option<i64>,
}
#[derive(Deserialize)]
pub struct WikiGEPrices {
    pub data: std::collections::HashMap<i32, WikiGEPrice>,
}
pub type GEPrices = std::collections::HashMap<i32, i64>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterAccount {
    pub username: String,
    pub password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginAccount {
    pub username: String,
    pub password: String,
}
/// Discord's redirect back to `/discord/callback` - not `deny_unknown_fields` since Discord
/// controls this query string, not us.
#[derive(Deserialize)]
pub struct DiscordCallbackQuery {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}
#[derive(Serialize)]
pub struct Account {
    pub id: i64,
    /// `None` for a Discord-only account that has never set a username/password.
    pub username: Option<String>,
    pub discord_name: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Set by an admin password reset; the frontend redirects to the password-change section
    /// as soon as it sees this on any authed response, rather than waiting for the 403 the
    /// gate returns on the next *other* authed endpoint.
    pub must_change_password: bool,
}
#[derive(Serialize)]
pub struct AuthenticatedAccount {
    pub account: Account,
    pub token: String,
    /// Only set on `register`/first-ever Discord login, the one time the raw API key is
    /// available to hand back - `login` always returns `None`, the key is never re-shown.
    pub api_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAccountUsername {
    pub username: String,
}
/// No `current_password` field - an active session is treated as sufficient proof of identity
/// for this mutation, same as `UpdateAccountUsername` above.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeAccountPassword {
    pub new_password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkCharacter {
    pub account_hash: String,
    pub rsn: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkCharacterToGroup {
    pub character_id: i64,
    pub group_name: String,
    pub group_token: String,
}
/// Mirrors the browser's `PushSubscriptionJSON` shape verbatim so the site can forward its
/// `pushManager.subscribe()` result straight through without reshaping it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribePush {
    pub endpoint: String,
    pub keys: PushSubscriptionKeys,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushSubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsubscribePush {
    pub endpoint: String,
}
#[derive(Serialize)]
pub struct CharacterGroupLink {
    pub character_id: i64,
    pub group_id: i64,
    pub linked_at: DateTime<Utc>,
}
impl From<crate::db::CharacterGroupLink> for CharacterGroupLink {
    fn from(link: crate::db::CharacterGroupLink) -> Self {
        CharacterGroupLink {
            character_id: link.character_id,
            group_id: link.group_id,
            linked_at: link.linked_at,
        }
    }
}
#[derive(Serialize)]
pub struct Character {
    pub id: i64,
    pub account_hash: String,
    pub display_rsn: String,
    pub bound_at: DateTime<Utc>,
    pub status: String,
    pub combat_level: Option<i16>,
    pub total_level: Option<i32>,
    /// A real "not in a group" signal only on the account-scoped character list (see
    /// `list_characters_for_account_with_group_status`), which joins against
    /// `character_group_links`. Every other endpoint that returns `Character` (confirm,
    /// remove-pending, unlink, link) always sends `None` here regardless of actual group
    /// status - they don't do that join, so absence isn't meaningful there.
    pub group_id: Option<i64>,
    /// Same "only meaningful on the account-scoped list" caveat as `group_id` above.
    pub group_name: Option<String>,
}
impl From<crate::db::Character> for Character {
    fn from(character: crate::db::Character) -> Self {
        Character {
            id: character.id,
            account_hash: character.account_hash,
            display_rsn: character.display_rsn,
            bound_at: character.bound_at,
            status: character.status,
            combat_level: character.combat_level,
            total_level: character.total_level,
            group_id: None,
            group_name: None,
        }
    }
}
impl From<crate::db::CharacterWithGroupStatus> for Character {
    fn from(entry: crate::db::CharacterWithGroupStatus) -> Self {
        let mut character = Character::from(entry.character);
        character.group_id = entry.group_id;
        character.group_name = entry.group_name;
        character
    }
}
/// Returned once from `register`/first Discord login and from `regenerate_api_key` - the raw
/// key is never re-shown after that (only its hash is persisted).
#[derive(Serialize)]
pub struct AccountApiKey {
    pub api_key: String,
}
/// Sent by the plugin independently of group-link status (see `character_identity_scope` in
/// main.rs) so a pending, not-yet-grouped character's RSN reaches the site's confirm card.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentifyCharacter {
    pub rsn: String,
    #[serde(default)]
    pub combat_level: Option<i16>,
    #[serde(default)]
    pub total_level: Option<i32>,
}

/// Per-member permission toggles, ported from `groupscape-old`'s `PermissionKey`
/// (`repositories/memberships.ts`). All keys default `false` for a new member (§6) - the
/// group admin's implicit all-permissions override lives outside this struct, computed from
/// `groups.admin_account_id` (see the "group admin has all permissions by default" ticket).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PermissionFlags {
    pub invite_members: bool,
    pub regenerate_group_key: bool,
    pub kick_members: bool,
    pub manage_settings: bool,
    pub manage_permissions: bool,
    pub post_map_markers: bool,
    pub post_callouts: bool,
    pub manage_goals: bool,
    pub manage_discord: bool,
    pub manage_events: bool,
}
impl PermissionFlags {
    pub fn all_true() -> Self {
        PermissionFlags {
            invite_members: true,
            regenerate_group_key: true,
            kick_members: true,
            manage_settings: true,
            manage_permissions: true,
            post_map_markers: true,
            post_callouts: true,
            manage_goals: true,
            manage_discord: true,
            manage_events: true,
        }
    }

    pub fn get(&self, key: PermissionKey) -> bool {
        match key {
            PermissionKey::InviteMembers => self.invite_members,
            PermissionKey::RegenerateGroupKey => self.regenerate_group_key,
            PermissionKey::KickMembers => self.kick_members,
            PermissionKey::ManageSettings => self.manage_settings,
            PermissionKey::ManagePermissions => self.manage_permissions,
            PermissionKey::PostMapMarkers => self.post_map_markers,
            PermissionKey::PostCallouts => self.post_callouts,
            PermissionKey::ManageGoals => self.manage_goals,
            PermissionKey::ManageDiscord => self.manage_discord,
            PermissionKey::ManageEvents => self.manage_events,
        }
    }
}

/// Response body for `/get-my-permissions`: the caller's effective flags plus their own member
/// name in this group (`None` if their account has no member row here yet). Lets the client
/// identify which roster row is "you" without a second round trip, e.g. to hide the
/// remove/block-self controls.
#[derive(Serialize, Debug)]
pub struct MyPermissions {
    pub member_name: Option<String>,
    #[serde(flatten)]
    pub flags: PermissionFlags,
}

/// One key per `PermissionFlags` toggle - lets callers reference a permission by name (e.g.
/// enforcement middleware) instead of reaching into the struct field directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionKey {
    InviteMembers,
    RegenerateGroupKey,
    KickMembers,
    ManageSettings,
    ManagePermissions,
    PostMapMarkers,
    PostCallouts,
    ManageGoals,
    ManageDiscord,
    ManageEvents,
}

/// Partial update over [`PermissionFlags`] - `None` per field leaves the stored value
/// untouched.
#[derive(Deserialize, Clone, Copy, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct PermissionFlagsPatch {
    #[serde(default)]
    pub invite_members: Option<bool>,
    #[serde(default)]
    pub regenerate_group_key: Option<bool>,
    #[serde(default)]
    pub kick_members: Option<bool>,
    #[serde(default)]
    pub manage_settings: Option<bool>,
    #[serde(default)]
    pub manage_permissions: Option<bool>,
    #[serde(default)]
    pub post_map_markers: Option<bool>,
    #[serde(default)]
    pub post_callouts: Option<bool>,
    #[serde(default)]
    pub manage_goals: Option<bool>,
    #[serde(default)]
    pub manage_discord: Option<bool>,
    #[serde(default)]
    pub manage_events: Option<bool>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct GroupPermissions {
    pub group_id: i64,
    pub account_id: i64,
    #[serde(flatten)]
    pub flags: PermissionFlags,
}

/// [`GroupPermissions`] plus the display name the permission-management UI lists a member
/// under - `group_permissions` only knows `account_id`, not any RSN, so this joins in the
/// most-recently-bound character's `display_rsn` for that account. `is_admin` marks the
/// group's implicit all-permissions holder (`flags` is still that account's real, mostly-false
/// stored row - the site renders admins as locked/all-on rather than trusting `flags` for them).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct GroupMemberPermissions {
    pub account_id: i64,
    pub display_rsn: String,
    pub is_admin: bool,
    /// `None` for an account whose linked character has no member row yet (e.g. joined before
    /// this feature existed and hasn't reconnected to trigger the backfill in
    /// `ensure_member_for_linked_character`).
    pub color: Option<String>,
    #[serde(flatten)]
    pub flags: PermissionFlags,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateMemberColorRequest {
    pub account_id: i64,
    pub color: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateGroupPermissionsRequest {
    pub account_id: i64,
    #[serde(flatten)]
    pub patch: PermissionFlagsPatch,
}

/// `webhook_url: None` disables dispatch (stored as `NULL`) rather than being an "unset,
/// leave alone" patch field like [`PermissionFlagsPatch`] - the discord settings form always
/// submits its full state, mirroring `groupscape-old`'s `PUT /groups/:groupId/discord`.
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct DiscordWebhookSettings {
    pub webhook_url: Option<String>,
    pub notify_kills: bool,
    pub notify_deaths: bool,
    pub notify_loot: bool,
}

#[derive(Deserialize)]
pub struct CaptchaVerifyResponse {
    pub success: bool,
    // NOTE: unused
    // #[serde(rename = "error-codes", default)]
    // pub error_codes: std::vec::Vec<String>,
}

fn default_admin_page() -> i64 {
    1
}
fn default_admin_page_size() -> i64 {
    25
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminGroupsQuery {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default = "default_admin_page")]
    pub page: i64,
    #[serde(default = "default_admin_page_size")]
    pub page_size: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminPageQuery {
    #[serde(default = "default_admin_page")]
    pub page: i64,
    #[serde(default = "default_admin_page_size")]
    pub page_size: i64,
}

#[derive(Serialize)]
pub struct AdminGroupSummary {
    pub group_id: i64,
    pub group_name: String,
    pub version: i32,
    pub member_count: i64,
    pub status: String,
}

#[derive(Serialize)]
pub struct AdminGroupsResponse {
    pub groups: Vec<AdminGroupSummary>,
    pub total: i64,
}

#[derive(Serialize)]
pub struct AdminGroupDetail {
    pub group_id: i64,
    pub group_name: String,
    pub version: i32,
    pub status: String,
    pub reason: Option<String>,
    pub members: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminModerationRequest {
    #[serde(default)]
    pub reason: Option<String>,
}


#[derive(Serialize)]
pub struct AdminAuditLogEntry {
    pub id: i64,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub detail: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct AdminAuditLogResponse {
    pub entries: Vec<AdminAuditLogEntry>,
    pub total: i64,
}

#[derive(Serialize)]
pub struct AdminAccountsSummary {
    pub count: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminAccountsQuery {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub group_id: Option<i64>,
    #[serde(default = "default_admin_page")]
    pub page: i64,
    #[serde(default = "default_admin_page_size")]
    pub page_size: i64,
}

#[derive(Serialize)]
pub struct AdminAccountSummary {
    pub id: i64,
    pub username: Option<String>,
    pub status: String,
    pub must_change_password: bool,
    pub locked_out: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct AdminAccountsResponse {
    pub accounts: Vec<AdminAccountSummary>,
    pub total: i64,
}

#[derive(Serialize)]
pub struct AdminAccountGroup {
    pub group_id: i64,
    pub group_name: String,
    pub is_owner: bool,
}

#[derive(Serialize)]
pub struct AdminAccountCharacter {
    pub id: i64,
    pub display_rsn: String,
    pub status: String,
    pub bound_at: DateTime<Utc>,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
}

#[derive(Serialize)]
pub struct AdminAccountSession {
    pub session_id: i64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Serialize)]
pub struct AdminAccountDetail {
    pub id: i64,
    pub username: Option<String>,
    pub status: String,
    pub must_change_password: bool,
    pub locked_out: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub groups: Vec<AdminAccountGroup>,
    pub characters: Vec<AdminAccountCharacter>,
    pub session_count: i64,
}

#[derive(Serialize)]
pub struct AdminPasswordResetResponse {
    pub temp_password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminSetAccountStatus {
    pub status: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminSetAccountUsername {
    pub username: String,
}

#[derive(Serialize)]
pub struct AdminSearchResponse {
    pub accounts: Vec<AdminAccountSummary>,
    pub groups: Vec<AdminGroupSummary>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminSearchQuery {
    pub q: String,
}

#[derive(Serialize)]
pub struct AdminDashboard {
    pub total: i64,
    pub active: i64,
    pub suspended: i64,
    pub banned: i64,
    pub deleted: i64,
    pub live_sessions: i64,
    pub locked_out: i64,
    pub recent_audit: Vec<AdminAuditLogEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_hp_alert_requires_interaction_and_names_the_member() {
        let alert = AlertEvent::LowHp(LowHpAlert {
            current_hp: 14,
            max_hp: 99,
            world_x: 0,
            world_y: 0,
            plane: 0,
            world: 0,
        });
        assert_eq!(alert.alert_type(), "low_hp");
        assert!(alert.requires_interaction());
        let (title, body) = alert.push_title_and_body("Woox");
        assert_eq!(title, "Low HP — Woox");
        assert_eq!(body, "14/99 HP remaining");
    }

    #[test]
    fn wilderness_entry_alert_auto_dismisses_and_names_the_member() {
        let alert = AlertEvent::WildernessEntry(WildernessEntryAlert {
            wilderness_level: 27,
            world_x: 0,
            world_y: 0,
            plane: 0,
            world: 0,
        });
        assert_eq!(alert.alert_type(), "wilderness_entry");
        assert!(!alert.requires_interaction());
        let (title, body) = alert.push_title_and_body("Woox");
        assert_eq!(title, "Wilderness entry — Woox");
        assert_eq!(body, "Entered level 27 wilderness");
    }
}
