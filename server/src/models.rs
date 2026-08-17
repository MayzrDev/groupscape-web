use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const SHARED_MEMBER: &str = "@SHARED";

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<DateTime<Utc>>,
    /// Discrete kill/death events piggybacked on the same heartbeat, matching
    /// `groupscape-old`'s "ride the tick" ingestion pattern rather than a separate endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<GameEvent>>,
}

/// One item entry in a [`GameEvent::Kill`]'s loot, field names matching the plugin's
/// `PendingKill.toMap()` output verbatim (`itemId`/`quantity`).
#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
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

#[derive(Serialize)]
pub struct ActivityEvent {
    pub id: i64,
    pub session_id: i64,
    pub member_name: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: serde_json::Value,
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
    pub email: String,
    pub password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginAccount {
    pub email: String,
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
    /// `None` for a Discord-only account that has never set an email/password.
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
}
#[derive(Serialize)]
pub struct AuthenticatedAccount {
    pub account: Account,
    pub token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAccountEmail {
    pub email: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeAccountPassword {
    pub current_password: String,
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
}
impl From<crate::db::Character> for Character {
    fn from(character: crate::db::Character) -> Self {
        Character {
            id: character.id,
            account_hash: character.account_hash,
            display_rsn: character.display_rsn,
            bound_at: character.bound_at,
        }
    }
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
/// untouched, same "only touch what's provided" shape as [`AdminSetFeatureFlag`].
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
pub struct AdminFeatureFlag {
    pub flag_key: String,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminSetFeatureFlag {
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
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
