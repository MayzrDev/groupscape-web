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
pub struct RenameGroupMember {
    pub original_name: String,
    pub new_name: String,
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
pub struct LinkCharacter {
    pub account_hash: String,
    pub rsn: String,
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
