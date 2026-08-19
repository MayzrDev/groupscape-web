use crate::crypto::token_hash;
use crate::error::ApiError;
use crate::models::{
    ActivityEvent, AdminAuditLogEntry, AdminFeatureFlag, AdminGroupDetail, AdminGroupSummary,
    AggregateSkillData, BlockedMember, CreateGroup, DialogueEvent, GameEvent, GroupMember,
    GroupMemberPermissions, GroupPermissions, GroupSession, GroupSkillData, MemberSkillData,
    ObjectInteractionEvent, PermissionFlags, PermissionFlagsPatch, PermissionKey, SHARED_MEMBER,
};
use crate::validators::valid_name;
use chrono::{DateTime, Utc};
use deadpool_postgres::{Client, Transaction};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{HashMap, HashSet};
use tokio_postgres::Row;

const CURRENT_GROUP_VERSION: i32 = 2;
pub async fn create_group(client: &mut Client, create_group: &CreateGroup) -> Result<(), ApiError> {
    let create_group_stmt = client.prepare_cached("INSERT INTO groupscape.groups (group_name, group_token_hash, version) VALUES($1, $2, $3) RETURNING group_id").await?;
    let create_member_stmt = client
        .prepare_cached("INSERT INTO groupscape.members (group_id, member_name) VALUES($1, $2)")
        .await?;
    let transaction = client.transaction().await?;

    let hashed_token = token_hash(&create_group.token, &create_group.name);
    let group_id: i64 = transaction
        .query_one(
            &create_group_stmt,
            &[&create_group.name, &hashed_token, &CURRENT_GROUP_VERSION],
        )
        .await?
        .try_get(0)
        .map_err(ApiError::GroupCreationError)?;

    transaction
        .execute(&create_member_stmt, &[&group_id, &SHARED_MEMBER])
        .await
        .map_err(ApiError::GroupCreationError)?;
    for member_name in &create_group.member_names {
        transaction
            .execute(&create_member_stmt, &[&group_id, &member_name])
            .await
            .map_err(ApiError::GroupCreationError)?;
    }

    transaction
        .commit()
        .await
        .map_err(ApiError::GroupCreationError)
}

pub async fn add_group_member(
    client: &Client,
    group_id: i64,
    member_name: &str,
) -> Result<(), ApiError> {
    let member_count_stmt = client
        .prepare_cached(
            "SELECT COUNT(*) FROM groupscape.members WHERE group_id=$1 AND member_name!=$2",
        )
        .await?;
    let member_count: i64 = client
        .query_one(&member_count_stmt, &[&group_id, &SHARED_MEMBER])
        .await?
        .try_get(0)
        .map_err(ApiError::AddMemberError)?;

    if member_count >= 5 {
        return Err(ApiError::GroupFullError);
    }

    let create_member_stmt = client
        .prepare_cached("INSERT INTO groupscape.members (group_id, member_name) VALUES($1, $2)")
        .await?;
    client
        .execute(&create_member_stmt, &[&group_id, &member_name])
        .await
        .map_err(ApiError::AddMemberError)?;
    Ok(())
}

/// Resolves the canonical member row for a plugin-submitted `account_hash`, ported from
/// `groupscape-old`'s telemetry-ingestion pattern (`account_hash` is the persistent identity,
/// `display_rsn`/`member_name` is just a mutable label kept in sync with it) and adapted to this
/// schema's separate `members` table (`characters.display_rsn` is the source of truth; `members`
/// gains its own `account_hash` column so a row survives an in-game name change instead of being
/// orphaned under the old name).
///
/// Lookup order:
/// 1. A member row already tagged with this `account_hash` in this group - rename it in place if
///    `display_rsn` has since changed.
/// 2. A pre-existing untagged row matching `display_rsn` by name (e.g. typed in at group setup
///    before this character was linked) - claim it by tagging its `account_hash`.
/// 3. Neither exists - insert a new row, subject to the same 5-member cap as `add_group_member`.
///
/// Returns the member name the caller should use for this update (== `display_rsn`).
pub async fn ensure_member_for_linked_character(
    client: &Client,
    group_id: i64,
    account_hash: &str,
    display_rsn: &str,
) -> Result<String, ApiError> {
    let by_hash_stmt = client
        .prepare_cached(
            "SELECT member_name FROM groupscape.members WHERE group_id=$1 AND account_hash=$2",
        )
        .await?;
    if let Some(row) = client
        .query_opt(&by_hash_stmt, &[&group_id, &account_hash])
        .await
        .map_err(ApiError::AddMemberError)?
    {
        let existing_name: String = row.try_get("member_name")?;
        if existing_name == display_rsn {
            return Ok(existing_name);
        }
        let rename_stmt = client
            .prepare_cached(
                "UPDATE groupscape.members SET member_name=$1 WHERE group_id=$2 AND account_hash=$3",
            )
            .await?;
        return match client
            .execute(&rename_stmt, &[&display_rsn, &group_id, &account_hash])
            .await
        {
            Ok(_) => Ok(display_rsn.to_string()),
            // member_name collision with an unrelated row (e.g. a stale untagged member still
            // squatting on the new name) - keep serving the old name rather than failing the
            // telemetry update outright.
            Err(_) => Ok(existing_name),
        };
    }

    let claim_stmt = client
        .prepare_cached(
            "UPDATE groupscape.members SET account_hash=$1 WHERE group_id=$2 AND member_name=$3 AND account_hash IS NULL",
        )
        .await?;
    let claimed = client
        .execute(&claim_stmt, &[&account_hash, &group_id, &display_rsn])
        .await
        .map_err(ApiError::AddMemberError)?;
    if claimed > 0 {
        return Ok(display_rsn.to_string());
    }

    let member_count_stmt = client
        .prepare_cached(
            "SELECT COUNT(*) FROM groupscape.members WHERE group_id=$1 AND member_name!=$2",
        )
        .await?;
    let member_count: i64 = client
        .query_one(&member_count_stmt, &[&group_id, &SHARED_MEMBER])
        .await?
        .try_get(0)
        .map_err(ApiError::AddMemberError)?;
    if member_count >= 5 {
        return Err(ApiError::GroupFullError);
    }

    let create_member_stmt = client
        .prepare_cached(
            "INSERT INTO groupscape.members (group_id, member_name, account_hash) VALUES($1, $2, $3)",
        )
        .await?;
    client
        .execute(
            &create_member_stmt,
            &[&group_id, &display_rsn, &account_hash],
        )
        .await
        .map_err(ApiError::AddMemberError)?;
    Ok(display_rsn.to_string())
}

pub async fn delete_skills_data_for_member(
    transaction: &Transaction<'_>,
    period: AggregatePeriod,
    member_id: i64,
) -> Result<(), ApiError> {
    let s = format!(
        r#"
DELETE FROM groupscape.skills_{} WHERE member_id=$1
"#,
        match period {
            AggregatePeriod::Day => "day",
            AggregatePeriod::Month => "month",
            AggregatePeriod::Year => "year",
        }
    );
    let delete_skills_data_stmt = transaction.prepare_cached(&s).await?;
    transaction
        .execute(&delete_skills_data_stmt, &[&member_id])
        .await?;

    Ok(())
}

pub async fn delete_collection_log_data_for_member(
    transaction: &Transaction<'_>,
    member_id: i64,
) -> Result<(), ApiError> {
    let a = "DELETE FROM groupscape.collection_log WHERE member_id=$1";
    let delete_collection_stmt = transaction.prepare_cached(a).await?;
    transaction
        .execute(&delete_collection_stmt, &[&member_id])
        .await?;

    let b = "DELETE FROM groupscape.collection_log_new WHERE member_id=$1";
    let delete_new_stmt = transaction.prepare_cached(b).await?;
    transaction.execute(&delete_new_stmt, &[&member_id]).await?;

    Ok(())
}

pub async fn get_member_id(
    client: &Client,
    group_id: i64,
    member_name: &str,
) -> Result<i64, ApiError> {
    let get_member_id_stmt = client
        .prepare_cached(
            "SELECT member_id FROM groupscape.members WHERE group_id=$1 AND member_name=$2",
        )
        .await?;
    let member_id: i64 = client
        .query_one(&get_member_id_stmt, &[&group_id, &member_name])
        .await
        .map_err(ApiError::DeleteGroupMemberError)?
        .try_get(0)?;
    Ok(member_id)
}

pub async fn delete_group_member(
    client: &mut Client,
    group_id: i64,
    member_name: &str,
) -> Result<(), ApiError> {
    let member_id = get_member_id(client, group_id, member_name).await?;
    let transaction = client.transaction().await?;
    delete_skills_data_for_member(&transaction, AggregatePeriod::Day, member_id).await?;
    delete_skills_data_for_member(&transaction, AggregatePeriod::Month, member_id).await?;
    delete_skills_data_for_member(&transaction, AggregatePeriod::Year, member_id).await?;
    delete_collection_log_data_for_member(&transaction, member_id).await?;

    let stmt = transaction
        .prepare_cached("DELETE FROM groupscape.members WHERE group_id=$1 AND member_name=$2")
        .await?;
    transaction
        .execute(&stmt, &[&group_id, &member_name])
        .await
        .map_err(ApiError::DeleteGroupMemberError)?;

    transaction
        .commit()
        .await
        .map_err(ApiError::DeleteGroupMemberError)?;

    Ok(())
}

pub async fn is_member_blocked(
    client: &Client,
    group_id: i64,
    member_name: &str,
) -> Result<bool, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT 1 FROM groupscape.blocked_members WHERE group_id=$1 AND member_name=$2",
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&group_id, &member_name])
        .await
        .map_err(ApiError::IsMemberBlockedError)?;
    Ok(row.is_some())
}

/// Auto-provisions a member the first time their telemetry arrives - there's no manual
/// "add member" step anymore, so joining the group happens implicitly on first update.
/// A previously-removed member rejoins the same way, since removal fully deletes their row.
/// A blocked member is rejected here instead, before any data is written or broadcast.
pub async fn ensure_group_member(
    client: &Client,
    group_id: i64,
    member_name: &str,
) -> Result<(), ApiError> {
    if is_member_blocked(client, group_id, member_name).await? {
        return Err(ApiError::MemberBlockedError);
    }

    if is_member_in_group(client, group_id, member_name).await? {
        return Ok(());
    }

    if !valid_name(member_name) {
        return Err(ApiError::GroupMemberValidationError(format!(
            "Member name {} is not valid",
            member_name
        )));
    }

    add_group_member(client, group_id, member_name).await
}

pub async fn get_blocked_members(
    client: &Client,
    group_id: i64,
) -> Result<Vec<BlockedMember>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT member_name, blocked_at FROM groupscape.blocked_members WHERE group_id=$1 ORDER BY blocked_at DESC",
        )
        .await?;
    let rows = client
        .query(&stmt, &[&group_id])
        .await
        .map_err(ApiError::GetBlockedMembersError)?;
    rows.iter()
        .map(|row| {
            Ok(BlockedMember {
                member_name: row.try_get("member_name")?,
                blocked_at: row.try_get("blocked_at")?,
            })
        })
        .collect()
}

/// Blocks a member: wipes their tracked data the same way Remove does (if they're
/// currently in the group) and records the block so future telemetry under that name is
/// rejected until unblocked.
pub async fn block_group_member(
    client: &mut Client,
    group_id: i64,
    member_name: &str,
) -> Result<(), ApiError> {
    if is_member_in_group(client, group_id, member_name).await? {
        delete_group_member(client, group_id, member_name).await?;
    }

    let stmt = client
        .prepare_cached(
            "INSERT INTO groupscape.blocked_members (group_id, member_name) VALUES ($1, $2) ON CONFLICT (group_id, member_name) DO NOTHING",
        )
        .await?;
    client
        .execute(&stmt, &[&group_id, &member_name])
        .await
        .map_err(ApiError::BlockGroupMemberError)?;
    Ok(())
}

pub async fn unblock_group_member(
    client: &Client,
    group_id: i64,
    member_name: &str,
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached(
            "DELETE FROM groupscape.blocked_members WHERE group_id=$1 AND member_name=$2",
        )
        .await?;
    client
        .execute(&stmt, &[&group_id, &member_name])
        .await
        .map_err(ApiError::UnblockGroupMemberError)?;
    Ok(())
}

pub async fn rename_group(
    client: &Client,
    group_id: i64,
    new_name: &str,
) -> Result<String, ApiError> {
    let new_token = format!("{}|{}", new_name, uuid::Uuid::new_v4().hyphenated());
    let hashed_token = token_hash(&new_token, new_name);
    let stmt = client
        .prepare_cached(
            "UPDATE groupscape.groups SET group_name=$1, group_token_hash=$2 WHERE group_id=$3",
        )
        .await?;
    client
        .execute(&stmt, &[&new_name, &hashed_token, &group_id])
        .await
        .map_err(ApiError::RenameGroupError)?;
    Ok(new_token)
}

pub async fn reroll_group_token(
    client: &Client,
    group_id: i64,
    group_name: &str,
) -> Result<String, ApiError> {
    let new_token = format!("{}|{}", group_name, uuid::Uuid::new_v4().hyphenated());
    let hashed_token = token_hash(&new_token, group_name);
    let stmt = client
        .prepare_cached("UPDATE groupscape.groups SET group_token_hash=$1 WHERE group_id=$2")
        .await?;
    client
        .execute(&stmt, &[&hashed_token, &group_id])
        .await
        .map_err(ApiError::RerollGroupTokenError)?;
    Ok(new_token)
}

pub struct HomepageStats {
    pub active_groups: i64,
    pub bound_characters: i64,
    pub online_characters: i64,
}

pub async fn get_homepage_stats(client: &Client) -> Result<HomepageStats, ApiError> {
    let active_groups: i64 = client
        .query_one("SELECT COUNT(*) FROM groupscape.groups", &[])
        .await?
        .try_get(0)?;

    let bound_characters: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM groupscape.members WHERE member_name != $1",
            &[&SHARED_MEMBER],
        )
        .await?
        .try_get(0)?;

    // Mirrors the 60s "online" threshold used for the live online badge in the group view.
    let online_characters: i64 = client
        .query_one(
            r#"
SELECT COUNT(*) FROM groupscape.members
WHERE member_name != $1 AND GREATEST(
    stats_last_update, coordinates_last_update, skills_last_update,
    quests_last_update, inventory_last_update, equipment_last_update, bank_last_update,
    rune_pouch_last_update, interacting_last_update, seed_vault_last_update, diary_vars_last_update,
    collection_log_last_update, potion_storage_last_update
) >= NOW() - INTERVAL '60 seconds'
"#,
            &[&SHARED_MEMBER],
        )
        .await?
        .try_get(0)?;

    Ok(HomepageStats {
        active_groups,
        bound_characters,
        online_characters,
    })
}

pub async fn is_member_in_group(
    client: &Client,
    group_id: i64,
    member_name: &str,
) -> Result<bool, ApiError> {
    let stmt = client.prepare_cached("SELECT COUNT(member_name) FROM groupscape.members WHERE group_id=$1 AND member_name=$2").await?;
    let member_count: i64 = client
        .query_one(&stmt, &[&group_id, &member_name])
        .await?
        .try_get(0)
        .map_err(ApiError::IsMemberInGroupError)?;
    Ok(member_count > 0)
}

pub async fn upsert_member_mesh(
    client: &Client,
    member_id: i64,
    mesh: &[u8],
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO groupscape.member_mesh (member_id, mesh, mesh_last_update) VALUES ($1, $2, NOW())
ON CONFLICT (member_id) DO UPDATE SET mesh=excluded.mesh, mesh_last_update=excluded.mesh_last_update
"#,
        )
        .await?;
    client
        .execute(&stmt, &[&member_id, &mesh])
        .await
        .map_err(ApiError::UpsertMemberMeshError)?;
    Ok(())
}

pub async fn get_member_mesh(
    client: &Client,
    group_id: i64,
    member_name: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
SELECT mm.mesh FROM groupscape.member_mesh mm
INNER JOIN groupscape.members m ON m.member_id=mm.member_id
WHERE m.group_id=$1 AND m.member_name=$2
"#,
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&group_id, &member_name])
        .await
        .map_err(ApiError::GetMemberMeshError)?;
    match row {
        Some(row) => Ok(Some(row.try_get("mesh")?)),
        None => Ok(None),
    }
}

pub fn serialize_serde<T>(value: &Option<T>) -> Result<Option<String>, ApiError>
where
    T: Serialize,
{
    match value {
        Some(v) => {
            let result = serde_json::to_string(&v)?;
            Ok(Some(result))
        }
        None => Ok(None),
    }
}

pub async fn get_group(client: &Client, group_name: &str, token: &str) -> Result<i64, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT group_id FROM groupscape.groups WHERE group_token_hash=$1 AND group_name=$2",
        )
        .await?;
    let hashed_token = token_hash(token, group_name);
    let group: Row = client
        .query_one(&stmt, &[&hashed_token, &group_name])
        .await
        .map_err(ApiError::GetGroupError)?;
    Ok(group.try_get(0)?)
}

fn try_deserialize_json_column<T>(row: &Row, column: &str) -> Result<Option<T>, ApiError>
where
    T: DeserializeOwned,
{
    match row.try_get(column) {
        Ok(column_data) => Ok(serde_json::from_str(column_data).ok()),
        Err(_) => Ok(None),
    }
}

pub async fn get_group_data(
    client: &Client,
    group_id: i64,
    timestamp: &DateTime<Utc>,
) -> Result<Vec<GroupMember>, ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
SELECT member_name,
GREATEST(stats_last_update, coordinates_last_update, skills_last_update,
quests_last_update, inventory_last_update, equipment_last_update, bank_last_update,
rune_pouch_last_update, interacting_last_update, seed_vault_last_update, diary_vars_last_update,
collection_log_last_update, potion_storage_last_update, special_attack_last_update,
active_prayers_last_update, rich_presence_last_update, combat_achievements_last_update) as last_updated,
CASE WHEN stats_last_update >= $1::TIMESTAMPTZ THEN stats ELSE NULL END as stats,
CASE WHEN coordinates_last_update >= $1::TIMESTAMPTZ THEN coordinates ELSE NULL END as coordinates,
CASE WHEN skills_last_update >= $1::TIMESTAMPTZ THEN skills ELSE NULL END as skills,
CASE WHEN quests_last_update >= $1::TIMESTAMPTZ THEN quests ELSE NULL END as quests,
CASE WHEN inventory_last_update >= $1::TIMESTAMPTZ THEN inventory ELSE NULL END as inventory,
CASE WHEN equipment_last_update >= $1::TIMESTAMPTZ THEN equipment ELSE NULL END as equipment,
CASE WHEN bank_last_update >= $1::TIMESTAMPTZ THEN bank ELSE NULL END as bank,
CASE WHEN rune_pouch_last_update >= $1::TIMESTAMPTZ THEN rune_pouch ELSE NULL END as rune_pouch,
CASE WHEN interacting_last_update >= $1::TIMESTAMPTZ THEN interacting ELSE NULL END as interacting,
CASE WHEN seed_vault_last_update >= $1::TIMESTAMPTZ THEN seed_vault ELSE NULL END as seed_vault,
CASE WHEN diary_vars_last_update >= $1::TIMESTAMPTZ THEN diary_vars ELSE NULL END as diary_vars,
CASE WHEN collection_log_last_update >= $1::TIMESTAMPTZ THEN collection_log ELSE NULL END as collection_log,
CASE WHEN potion_storage_last_update >= $1::TIMESTAMPTZ THEN potion_storage ELSE NULL END as potion_storage,
CASE WHEN special_attack_last_update >= $1::TIMESTAMPTZ THEN special_attack ELSE NULL END as special_attack,
CASE WHEN active_prayers_last_update >= $1::TIMESTAMPTZ THEN active_prayers ELSE NULL END as active_prayers,
CASE WHEN rich_presence_last_update >= $1::TIMESTAMPTZ THEN rich_presence ELSE NULL END as rich_presence,
CASE WHEN combat_achievements_last_update >= $1::TIMESTAMPTZ THEN combat_achievements ELSE NULL END as combat_achievements
FROM groupscape.members WHERE group_id=$2
"#,
        )
        .await?;

    let rows = client
        .query(&stmt, &[&timestamp, &group_id])
        .await
        .map_err(ApiError::GetGroupDataError)?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let member_name = row.try_get("member_name")?;
        let last_updated: Option<DateTime<Utc>> = row.try_get("last_updated").ok();
        let group_member = GroupMember {
            group_id: Some(group_id),
            name: member_name,
            account_hash: None,
            last_updated,
            stats: row.try_get("stats").ok(),
            coordinates: row.try_get("coordinates").ok(),
            skills: row.try_get("skills").ok(),
            quests: row.try_get("quests")?,
            inventory: row.try_get("inventory").ok(),
            equipment: row.try_get("equipment").ok(),
            bank: row.try_get("bank").ok(),
            rune_pouch: row.try_get("rune_pouch").ok(),
            seed_vault: row.try_get("seed_vault").ok(),
            interacting: try_deserialize_json_column(&row, "interacting")?,
            diary_vars: row.try_get("diary_vars").ok(),
            shared_bank: Option::None,
            deposited: Option::None,
            collection_log_v2: row.try_get("collection_log").ok(),
            potion_storage: row.try_get("potion_storage").ok(),
            special_attack: row.try_get("special_attack").ok(),
            active_prayers: row.try_get("active_prayers").ok(),
            rich_presence: row.try_get("rich_presence").ok(),
            events: None,
            interactions: None,
            object_interactions: None,
            combat_achievements: try_deserialize_json_column(&row, "combat_achievements")?,
        };
        result.push(group_member);
    }

    Ok(result)
}

/// Assigns each non-shared group member a stable display color by join order
/// (member_id ascending), matching groupscape-old's join-order palette so the
/// overlay's ownership stripe stays consistent across sessions.
pub async fn get_member_color_map(
    client: &Client,
    group_id: i64,
) -> Result<HashMap<String, String>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT member_name FROM groupscape.members WHERE group_id=$1 AND member_name != $2 ORDER BY member_id ASC",
        )
        .await?;
    let rows = client
        .query(&stmt, &[&group_id, &SHARED_MEMBER])
        .await
        .map_err(ApiError::GetMemberColorsError)?;

    let mut colors = HashMap::new();
    for (index, row) in rows.into_iter().enumerate() {
        let member_name: String = row.try_get(0)?;
        colors.insert(member_name, crate::websocket::member_color(index));
    }
    Ok(colors)
}

pub enum AggregatePeriod {
    Day,
    Month,
    Year,
}
async fn aggregate_skills_for_period(
    transaction: &Transaction<'_>,
    period: AggregatePeriod,
    last_aggregation: &DateTime<Utc>,
) -> Result<(), ApiError> {
    let s = format!(
        r#"
INSERT INTO groupscape.skills_{} (member_id, time, skills)
SELECT member_id, date_trunc('{}', skills_last_update), skills FROM groupscape.members
WHERE skills_last_update IS NOT NULL AND skills IS NOT NULL AND skills_last_update >= $1
ON CONFLICT (member_id, time)
DO UPDATE SET skills=excluded.skills;
"#,
        match period {
            AggregatePeriod::Day => "day",
            AggregatePeriod::Month => "month",
            AggregatePeriod::Year => "year",
        },
        match period {
            AggregatePeriod::Day => "hour",
            AggregatePeriod::Month => "day",
            AggregatePeriod::Year => "month",
        }
    );
    let aggregate_stmt = transaction.prepare_cached(&s).await?;
    transaction
        .execute(&aggregate_stmt, &[&last_aggregation])
        .await?;

    Ok(())
}

async fn apply_skills_retention_for_period(
    transaction: &Transaction<'_>,
    period: AggregatePeriod,
    last_aggregation: &DateTime<Utc>,
) -> Result<(), ApiError> {
    let s = format!(
        r#"
DELETE FROM groupscape.skills_{0}
WHERE time < ($1::timestamptz - interval '{1}') AND (member_id, time) NOT IN (
  SELECT member_id, max(time) FROM groupscape.skills_{0} WHERE time < ($1::timestamptz - interval '{1}') GROUP BY member_id
)
"#,
        match period {
            AggregatePeriod::Day => "day",
            AggregatePeriod::Month => "month",
            AggregatePeriod::Year => "year",
        },
        match period {
            AggregatePeriod::Day => "1 day",
            AggregatePeriod::Month => "1 month",
            AggregatePeriod::Year => "1 year",
        }
    );
    let delete_old_rows_stmt = transaction.prepare_cached(&s).await?;
    transaction
        .execute(&delete_old_rows_stmt, &[&last_aggregation])
        .await?;

    Ok(())
}

pub async fn get_last_skills_aggregation(client: &Client) -> Result<DateTime<Utc>, ApiError> {
    let last_aggregation_stmt = client
        .prepare_cached(
            r#"
SELECT last_aggregation FROM groupscape.aggregation_info WHERE type='skills'"#,
        )
        .await?;
    let last_aggregation: DateTime<Utc> = client
        .query_one(&last_aggregation_stmt, &[])
        .await?
        .try_get(0)?;

    Ok(last_aggregation)
}

pub async fn aggregate_skills(client: &mut Client) -> Result<(), ApiError> {
    let last_aggregation = get_last_skills_aggregation(client).await?;

    let transaction = client.transaction().await?;
    let update_last_aggregation_stmt = transaction
        .prepare_cached(
            r#"
UPDATE groupscape.aggregation_info SET last_aggregation=NOW() WHERE type='skills'"#,
        )
        .await?;
    transaction
        .execute(&update_last_aggregation_stmt, &[])
        .await?;

    aggregate_skills_for_period(&transaction, AggregatePeriod::Day, &last_aggregation).await?;
    aggregate_skills_for_period(&transaction, AggregatePeriod::Month, &last_aggregation).await?;
    aggregate_skills_for_period(&transaction, AggregatePeriod::Year, &last_aggregation).await?;
    transaction.commit().await?;

    Ok(())
}

pub async fn apply_skills_retention(client: &mut Client) -> Result<(), ApiError> {
    let last_aggregation = get_last_skills_aggregation(client).await?;

    let transaction = client.transaction().await?;
    apply_skills_retention_for_period(&transaction, AggregatePeriod::Day, &last_aggregation)
        .await?;
    apply_skills_retention_for_period(&transaction, AggregatePeriod::Month, &last_aggregation)
        .await?;
    apply_skills_retention_for_period(&transaction, AggregatePeriod::Year, &last_aggregation)
        .await?;
    transaction.commit().await?;

    Ok(())
}

pub async fn get_skills_for_period(
    client: &Client,
    group_id: i64,
    period: AggregatePeriod,
) -> Result<GroupSkillData, ApiError> {
    let s = format!(
        r#"
SELECT member_name, time, s.skills
FROM groupscape.skills_{} s
INNER JOIN groupscape.members m ON m.member_id=s.member_id
WHERE m.group_id=$1
"#,
        match period {
            AggregatePeriod::Day => "day",
            AggregatePeriod::Month => "month",
            AggregatePeriod::Year => "year",
        }
    );
    let get_skills_stmt = client.prepare_cached(&s).await?;
    let rows = client
        .query(&get_skills_stmt, &[&group_id])
        .await
        .map_err(ApiError::GetSkillsDataError)?;

    let mut member_data = HashMap::new();
    for row in rows {
        let member_name: String = row.try_get("member_name")?;
        let skill_data = AggregateSkillData {
            time: row.try_get("time")?,
            data: row.try_get("skills")?,
        };

        if !member_data.contains_key(&member_name) {
            member_data.insert(
                member_name.clone(),
                MemberSkillData {
                    name: member_name,
                    skill_data: vec![skill_data],
                },
            );
        } else if let Some(member_skill_data) = member_data.get_mut(&member_name) {
            member_skill_data.skill_data.push(skill_data);
        }
    }

    Ok(member_data.into_values().collect())
}

pub async fn has_migration_run(client: &mut Client, name: &str) -> Result<bool, ApiError> {
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM groupscape.migrations WHERE name=$1",
            &[&name],
        )
        .await?
        .try_get(0)?;

    Ok(count > 0)
}

pub async fn commit_migration(transaction: &Transaction<'_>, name: &str) -> Result<(), ApiError> {
    transaction
        .execute(
            "INSERT INTO groupscape.migrations (name, date) VALUES($1, NOW())",
            &[&name],
        )
        .await?;

    Ok(())
}

async fn create_timestamp_trigger(
    transaction: &Transaction<'_>,
    name: &str,
) -> Result<(), ApiError> {
    let create_fn = format!(
        r#"
CREATE OR REPLACE FUNCTION groupscape.update_{0}_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.{0}_last_update = now();
    RETURN NEW;
END;
$$ language 'plpgsql';
"#,
        name
    );
    transaction.execute(&create_fn, &[]).await?;

    let trigger_stmt = format!(
        r#"
DO
$$BEGIN
  CREATE TRIGGER set_{0}_timestamp
  BEFORE UPDATE ON groupscape.members
  FOR EACH ROW
  WHEN (OLD.{0} IS DISTINCT FROM NEW.{0})
  EXECUTE FUNCTION groupscape.update_{0}_timestamp();
EXCEPTION
  WHEN duplicate_object THEN
    NULL;
END;$$;
"#,
        name
    );
    transaction.execute(&trigger_stmt, &[]).await?;

    Ok(())
}

pub async fn update_schema(client: &mut Client) -> Result<(), ApiError> {
    client
        .execute(
            r#"
CREATE TABLE IF NOT EXISTS groupscape.migrations (
    name TEXT,
    date TIMESTAMPTZ
)
"#,
            &[],
        )
        .await?;

    if !has_migration_run(client, "add_groups_version_column").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.groups ADD COLUMN IF NOT EXISTS version INTEGER default 1
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "add_groups_version_column").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_members_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.members (
  member_id BIGSERIAL PRIMARY KEY,
  group_id BIGSERIAL REFERENCES groupscape.groups(group_id),
  member_name TEXT NOT NULL,

  stats_last_update TIMESTAMPTZ,
  stats INTEGER[7],

  coordinates_last_update TIMESTAMPTZ,
  coordinates INTEGER[3],

  skills_last_update TIMESTAMPTZ,
  skills INTEGER[24],

  quests_last_update TIMESTAMPTZ,
  quests bytea,

  inventory_last_update TIMESTAMPTZ,
  inventory INTEGER[56],

  equipment_last_update TIMESTAMPTZ,
  equipment INTEGER[28],

  rune_pouch_last_update TIMESTAMPTZ,
  rune_pouch INTEGER[8],

  bank_last_update TIMESTAMPTZ,
  bank INTEGER[],

  seed_vault_last_update TIMESTAMPTZ,
  seed_vault INTEGER[],

  interacting_last_update TIMESTAMPTZ,
  interacting TEXT
);
"#,
                &[],
            )
            .await?;

        transaction.execute(r#"
CREATE UNIQUE INDEX IF NOT EXISTS members_groupid_name_idx ON groupscape.members (group_id, member_name);
"#, &[]).await?;

        commit_migration(&transaction, "create_members_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_diary_vars").await? {
        let transaction = client.transaction().await?;
        // Adding new columns for new types of data
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.members
ADD COLUMN IF NOT EXISTS diary_vars_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS diary_vars INTEGER[62]
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "add_diary_vars").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_skill_periods").await? {
        let transaction = client.transaction().await?;

        let periods = vec!["day", "month", "year"];
        for period in periods {
            let create_skills_aggregate = format!(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.skills_{} (
    member_id BIGSERIAL REFERENCES groupscape.members(member_id),
    time TIMESTAMPTZ,
    skills INTEGER[24],

    PRIMARY KEY (member_id, time)
);
"#,
                period
            );
            transaction.execute(&create_skills_aggregate, &[]).await?;
        }

        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.aggregation_info (
    type TEXT PRIMARY KEY,
    last_aggregation TIMESTAMPTZ NOT NULL DEFAULT TIMESTAMP WITH TIME ZONE 'epoch'
);
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
INSERT INTO groupscape.aggregation_info (type) VALUES ('skills')
ON CONFLICT (type) DO NOTHING
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "add_skill_periods").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "member_name_citext").await? {
        let transaction = client.transaction().await?;

        // We need to rename members in groups which would violate the unique constraint after
        // we make the column case insensitive.
        let duplicates = transaction
            .query(
                r#"
SELECT a.group_id, a.member_id, a.member_name FROM groupscape.members a
INNER JOIN (
	SELECT group_id, lower(member_name) as member_name, COUNT(*) FROM groupscape.members
	GROUP BY group_id, lower(member_name)
	HAVING COUNT(*) > 1
) b
ON a.group_id=b.group_id AND lower(a.member_name)=lower(b.member_name)
ORDER BY GREATEST(
	stats_last_update,
	coordinates_last_update,
	skills_last_update,
	quests_last_update,
	inventory_last_update,
	equipment_last_update,
	bank_last_update,
	rune_pouch_last_update,
	interacting_last_update,
	seed_vault_last_update,
	diary_vars_last_update
) ASC;
"#,
                &[],
            )
            .await?;

        let mut already_encounted: HashSet<String> = HashSet::new();
        for row in duplicates {
            let group_id: i64 = row.try_get("group_id")?;
            let member_id: i64 = row.try_get("member_id")?;
            let member_name: String = row.try_get("member_name")?;
            let member_name_lower: String = member_name.to_lowercase();

            let key = format!("{}::{}", group_id, member_name_lower);
            // Skip the first encounter with the duplicate name since that is the entry
            // with the most recent update.
            if !already_encounted.insert(key) {
                log::info!(
                    "Renaming duplicate member name '{}' in group '{}'",
                    member_name,
                    group_id
                );

                for _ in 1..5 {
                    let uuid = uuid::Uuid::new_v4().hyphenated().to_string();
                    let new_name = &uuid[..uuid.find("-").unwrap()];
                    log::info!("Trying new name '{}'", new_name);
                    if transaction
                        .execute(
                            "UPDATE groupscape.members SET member_name=$1 WHERE member_id=$2",
                            &[&new_name, &member_id],
                        )
                        .await
                        .is_ok()
                    {
                        break;
                    }
                }
            }
        }

        transaction
            .execute(
                "CREATE EXTENSION IF NOT EXISTS citext WITH SCHEMA public",
                &[],
            )
            .await
            .ok();
        transaction
            .execute(
                "ALTER TABLE groupscape.members ALTER COLUMN member_name TYPE citext",
                &[],
            )
            .await?;

        commit_migration(&transaction, "member_name_citext").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_collection_log_member_column").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.members
ADD COLUMN IF NOT EXISTS collection_log_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS collection_log INTEGER[]
"#,
                &[],
            )
            .await?;
        commit_migration(&transaction, "add_collection_log_member_column").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "migrate_collection_log_v2").await?
        && has_migration_run(client, "add_collection_log").await?
    {
        println!("beginning migration migrate_collection_log_v2");
        let transaction = client.transaction().await?;

        // collect the data to migrate
        let rows = transaction
            .query("SELECT member_id, items FROM groupscape.collection_log WHERE cardinality(items) > 0", &[])
            .await
            .unwrap();
        let mut member_data: HashMap<i64, Vec<i32>> = HashMap::new();
        for row in rows {
            let member_id: i64 = row.try_get("member_id")?;
            let items: Vec<i32> = row.try_get("items")?;

            match member_data.get_mut(&member_id) {
                Some(collection_log) => {
                    collection_log.extend(items.iter());
                }
                None => {
                    member_data.insert(member_id, items);
                }
            };
        }
        println!("need to migrate {} members", member_data.len());

        // breakup into chunks
        let chunk_size = 100;
        let member_data_list: Vec<(i64, Vec<i32>)> = member_data.into_iter().collect();
        let mut chunks = Vec::new();
        for chunk_slice in member_data_list.chunks(chunk_size) {
            let chunk_map: HashMap<i64, Vec<i32>> = chunk_slice.iter().cloned().collect();
            chunks.push(chunk_map);
        }
        println!("split into {} chunks of size {}", chunks.len(), chunk_size);

        // update new collection log column
        for (i, chunk) in chunks.iter().enumerate() {
            println!(
                "migrating chunk {}/{} size {}",
                i + 1,
                chunks.len(),
                chunk.len()
            );
            let mut values_clause = String::new();
            for i in 0..chunk.len() {
                values_clause.push_str(&format!(
                    "(${}::BIGINT, ${}::INTEGER[])",
                    i * 2 + 1,
                    i * 2 + 2
                ));
                if i < chunk.len() - 1 {
                    values_clause.push_str(", ");
                }
            }
            let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
            for (member_id, items) in chunk.iter() {
                params.push(member_id);
                params.push(items);
            }

            // timestamp is set to value that will return on the initial frontend request, but does not show the player as online
            let update_query = format!(
                r#"
UPDATE groupscape.members as a SET collection_log=b.collection_log, collection_log_last_update='epoch'::timestamptz + INTERVAL '5 days'
FROM (VALUES {}) AS b(member_id, collection_log)
WHERE a.member_id=b.member_id
"#,
                values_clause
            );

            transaction.execute(&update_query, &params).await?;
        }

        commit_migration(&transaction, "migrate_collection_log_v2").await?;
        transaction.commit().await?;
        println!("finished migration migrate_collection_log_v2");
    }

    if !has_migration_run(client, "update_timestamp_triggers").await? {
        let transaction = client.transaction().await?;

        let names = vec![
            "stats",
            "coordinates",
            "skills",
            "quests",
            "inventory",
            "equipment",
            "bank",
            "rune_pouch",
            "interacting",
            "seed_vault",
            "diary_vars",
            "collection_log",
        ];

        for name in names {
            create_timestamp_trigger(&transaction, name).await?;
        }

        commit_migration(&transaction, "update_timestamp_triggers").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_potion_storage").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.members
ADD COLUMN IF NOT EXISTS potion_storage_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS potion_storage INTEGER[]
"#,
                &[],
            )
            .await?;

        create_timestamp_trigger(&transaction, "potion_storage").await?;

        commit_migration(&transaction, "add_potion_storage").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_party_overlay_columns").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.members
ADD COLUMN IF NOT EXISTS special_attack_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS special_attack INTEGER,
ADD COLUMN IF NOT EXISTS active_prayers_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS active_prayers TEXT[],
ADD COLUMN IF NOT EXISTS rich_presence_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS rich_presence TEXT
"#,
                &[],
            )
            .await?;

        create_timestamp_trigger(&transaction, "special_attack").await?;
        create_timestamp_trigger(&transaction, "active_prayers").await?;
        create_timestamp_trigger(&transaction, "rich_presence").await?;

        commit_migration(&transaction, "add_party_overlay_columns").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_member_mesh_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.member_mesh (
  member_id BIGINT PRIMARY KEY REFERENCES groupscape.members(member_id) ON DELETE CASCADE,
  mesh BYTEA NOT NULL,
  mesh_last_update TIMESTAMPTZ NOT NULL
);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_member_mesh_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_group_moderation_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.group_moderation (
  group_id BIGINT PRIMARY KEY REFERENCES groupscape.groups(group_id) ON DELETE CASCADE,
  status TEXT NOT NULL DEFAULT 'active',
  reason TEXT,
  actor TEXT NOT NULL DEFAULT 'admin',
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_group_moderation_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_feature_flags_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.feature_flags (
  flag_key TEXT PRIMARY KEY,
  enabled BOOLEAN NOT NULL DEFAULT false,
  description TEXT,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_feature_flags_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_admin_audit_log_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.admin_audit_log (
  id BIGSERIAL PRIMARY KEY,
  action TEXT NOT NULL,
  target_type TEXT,
  target_id TEXT,
  detail JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
CREATE INDEX IF NOT EXISTS admin_audit_log_created_at_idx ON groupscape.admin_audit_log(created_at DESC);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_admin_audit_log_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_accounts_stub_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.accounts (
  id BIGSERIAL PRIMARY KEY,
  email CITEXT UNIQUE,
  disabled BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_accounts_stub_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_account_password_hash").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.accounts ADD COLUMN IF NOT EXISTS password_hash TEXT
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "add_account_password_hash").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_account_sessions_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.account_sessions (
  session_id BIGSERIAL PRIMARY KEY,
  account_id BIGINT NOT NULL REFERENCES groupscape.accounts(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL
);
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
CREATE UNIQUE INDEX IF NOT EXISTS account_sessions_token_hash_idx ON groupscape.account_sessions (token_hash);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_account_sessions_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_account_discord_id").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.accounts ADD COLUMN IF NOT EXISTS discord_id TEXT
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
CREATE UNIQUE INDEX IF NOT EXISTS accounts_discord_id_idx ON groupscape.accounts (discord_id) WHERE discord_id IS NOT NULL;
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "add_account_discord_id").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_characters_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.characters (
  character_id BIGSERIAL PRIMARY KEY,
  account_id BIGINT NOT NULL REFERENCES groupscape.accounts(id) ON DELETE CASCADE,
  account_hash TEXT NOT NULL UNIQUE,
  display_rsn TEXT NOT NULL,
  bound_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
CREATE INDEX IF NOT EXISTS characters_account_id_idx ON groupscape.characters (account_id);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_characters_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_character_group_links_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.character_group_links (
  character_id BIGINT PRIMARY KEY REFERENCES groupscape.characters(character_id) ON DELETE CASCADE,
  group_id BIGINT NOT NULL REFERENCES groupscape.groups(group_id),
  linked_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
CREATE INDEX IF NOT EXISTS character_group_links_group_id_idx ON groupscape.character_group_links (group_id);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_character_group_links_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_blocked_members_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.blocked_members (
  group_id BIGINT NOT NULL REFERENCES groupscape.groups(group_id) ON DELETE CASCADE,
  member_name CITEXT NOT NULL,
  blocked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (group_id, member_name)
);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_blocked_members_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_groups_admin_account_id").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.groups
ADD COLUMN IF NOT EXISTS admin_account_id BIGINT REFERENCES groupscape.accounts(id)
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "add_groups_admin_account_id").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_members_account_hash_column").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.members ADD COLUMN IF NOT EXISTS account_hash TEXT
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
CREATE UNIQUE INDEX IF NOT EXISTS members_groupid_account_hash_idx ON groupscape.members (group_id, account_hash) WHERE account_hash IS NOT NULL
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "add_members_account_hash_column").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_group_permissions_table").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.group_permissions (
  group_id BIGINT NOT NULL REFERENCES groupscape.groups(group_id) ON DELETE CASCADE,
  account_id BIGINT NOT NULL REFERENCES groupscape.accounts(id) ON DELETE CASCADE,
  invite_members BOOLEAN NOT NULL DEFAULT false,
  regenerate_group_key BOOLEAN NOT NULL DEFAULT false,
  kick_members BOOLEAN NOT NULL DEFAULT false,
  manage_settings BOOLEAN NOT NULL DEFAULT false,
  manage_permissions BOOLEAN NOT NULL DEFAULT false,
  post_map_markers BOOLEAN NOT NULL DEFAULT false,
  post_callouts BOOLEAN NOT NULL DEFAULT false,
  manage_goals BOOLEAN NOT NULL DEFAULT false,
  manage_discord BOOLEAN NOT NULL DEFAULT false,
  manage_events BOOLEAN NOT NULL DEFAULT false,
  PRIMARY KEY (group_id, account_id)
);
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_group_permissions_table").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "create_sessions_and_activity_events_tables").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.sessions (
  session_id BIGSERIAL PRIMARY KEY,
  group_id BIGINT NOT NULL REFERENCES groupscape.groups(group_id) ON DELETE CASCADE,
  started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  ended_at TIMESTAMPTZ
);
"#,
                &[],
            )
            .await?;
        // Partial unique index (rather than a plain UNIQUE(group_id)) is what makes
        // `ensure_open_session`'s ON CONFLICT upsert atomic while still allowing a group to
        // accumulate many *closed* sessions over time.
        transaction
            .execute(
                r#"
CREATE UNIQUE INDEX IF NOT EXISTS sessions_one_open_per_group_idx ON groupscape.sessions (group_id) WHERE ended_at IS NULL
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
CREATE TABLE IF NOT EXISTS groupscape.activity_events (
  event_id BIGSERIAL PRIMARY KEY,
  session_id BIGINT NOT NULL REFERENCES groupscape.sessions(session_id) ON DELETE CASCADE,
  group_id BIGINT NOT NULL REFERENCES groupscape.groups(group_id) ON DELETE CASCADE,
  member_name CITEXT NOT NULL,
  event_type TEXT NOT NULL,
  occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  payload JSONB NOT NULL
);
"#,
                &[],
            )
            .await?;
        transaction
            .execute(
                r#"
CREATE INDEX IF NOT EXISTS activity_events_group_occurred_idx ON groupscape.activity_events (group_id, occurred_at DESC)
"#,
                &[],
            )
            .await?;

        commit_migration(&transaction, "create_sessions_and_activity_events_tables").await?;
        transaction.commit().await?;
    }

    if !has_migration_run(client, "add_combat_achievements_column").await? {
        let transaction = client.transaction().await?;
        transaction
            .execute(
                r#"
ALTER TABLE groupscape.members
ADD COLUMN IF NOT EXISTS combat_achievements_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS combat_achievements TEXT
"#,
                &[],
            )
            .await?;

        create_timestamp_trigger(&transaction, "combat_achievements").await?;

        commit_migration(&transaction, "add_combat_achievements_column").await?;
        transaction.commit().await?;
    }

    Ok(())
}

pub struct AccountForAuth {
    pub id: i64,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub disabled: bool,
    pub created_at: DateTime<Utc>,
}

impl From<AccountForAuth> for crate::models::Account {
    fn from(account: AccountForAuth) -> Self {
        crate::models::Account {
            id: account.id,
            email: account.email,
            created_at: account.created_at,
        }
    }
}

/// One account per user - `email` is a case-insensitive (citext) UNIQUE column, so a duplicate
/// registration surfaces as a Postgres unique-violation (SQLSTATE 23505) rather than needing a
/// separate existence check that would race with a concurrent registration of the same email.
/// Creates a Discord-only account (`email`/`password_hash` both left `NULL`) - matches
/// `groupscape-old`'s OAuth-first decision (grilled during that project's Slice 29): a Discord
/// id with no matching account auto-creates one rather than requiring a prior email signup.
pub async fn create_account_with_discord_id(
    client: &Client,
    discord_id: &str,
) -> Result<i64, ApiError> {
    let stmt = client
        .prepare_cached("INSERT INTO groupscape.accounts (discord_id) VALUES ($1) RETURNING id")
        .await?;
    let row = client
        .query_one(&stmt, &[&discord_id])
        .await
        .map_err(ApiError::CreateAccountError)?;
    Ok(row.try_get(0)?)
}

pub async fn get_account_by_discord_id(
    client: &Client,
    discord_id: &str,
) -> Result<Option<AccountForAuth>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT id, email, password_hash, disabled, created_at FROM groupscape.accounts WHERE discord_id=$1",
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&discord_id])
        .await
        .map_err(ApiError::GetAccountError)?;
    match row {
        Some(row) => Ok(Some(AccountForAuth {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            password_hash: row.try_get("password_hash")?,
            disabled: row.try_get("disabled")?,
            created_at: row.try_get("created_at")?,
        })),
        None => Ok(None),
    }
}

pub async fn create_account(
    client: &Client,
    email: &str,
    password_hash: &str,
) -> Result<i64, ApiError> {
    let stmt = client
        .prepare_cached(
            "INSERT INTO groupscape.accounts (email, password_hash) VALUES ($1, $2) RETURNING id",
        )
        .await?;
    match client.query_one(&stmt, &[&email, &password_hash]).await {
        Ok(row) => Ok(row.try_get(0)?),
        Err(err) => {
            if err.as_db_error().is_some_and(|db_err| {
                db_err.code() == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION
            }) {
                Err(ApiError::EmailAlreadyRegisteredError)
            } else {
                Err(ApiError::CreateAccountError(err))
            }
        }
    }
}

pub async fn get_account_by_email(
    client: &Client,
    email: &str,
) -> Result<Option<AccountForAuth>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT id, email, password_hash, disabled, created_at FROM groupscape.accounts WHERE email=$1",
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&email])
        .await
        .map_err(ApiError::GetAccountError)?;
    match row {
        Some(row) => Ok(Some(AccountForAuth {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            password_hash: row.try_get("password_hash")?,
            disabled: row.try_get("disabled")?,
            created_at: row.try_get("created_at")?,
        })),
        None => Ok(None),
    }
}

pub async fn get_account_by_id(
    client: &Client,
    account_id: i64,
) -> Result<Option<AccountForAuth>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT id, email, password_hash, disabled, created_at FROM groupscape.accounts WHERE id=$1",
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&account_id])
        .await
        .map_err(ApiError::GetAccountError)?;
    match row {
        Some(row) => Ok(Some(AccountForAuth {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            password_hash: row.try_get("password_hash")?,
            disabled: row.try_get("disabled")?,
            created_at: row.try_get("created_at")?,
        })),
        None => Ok(None),
    }
}

/// `email` is `citext UNIQUE`, so a duplicate update surfaces as a unique-violation same as
/// `create_account` above.
pub async fn update_account_email(
    client: &Client,
    account_id: i64,
    email: &str,
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached("UPDATE groupscape.accounts SET email=$1 WHERE id=$2")
        .await?;
    match client.execute(&stmt, &[&email, &account_id]).await {
        Ok(_) => Ok(()),
        Err(err) => {
            if err.as_db_error().is_some_and(|db_err| {
                db_err.code() == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION
            }) {
                Err(ApiError::EmailAlreadyRegisteredError)
            } else {
                Err(ApiError::UpdateAccountEmailError(err))
            }
        }
    }
}

pub async fn update_account_password(
    client: &Client,
    account_id: i64,
    password_hash: &str,
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached("UPDATE groupscape.accounts SET password_hash=$1 WHERE id=$2")
        .await?;
    client
        .execute(&stmt, &[&password_hash, &account_id])
        .await
        .map_err(ApiError::UpdateAccountPasswordError)?;
    Ok(())
}

/// Hard delete - `characters` (`ON DELETE CASCADE` from `accounts`), `character_group_links`
/// (cascades again from `characters`), and `account_sessions` (`ON DELETE CASCADE` from
/// `accounts`) all clean up via existing FK constraints, so a single row delete is enough.
/// Groups themselves are untouched: group ownership isn't tracked against accounts yet.
pub async fn delete_account(client: &Client, account_id: i64) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached("DELETE FROM groupscape.accounts WHERE id=$1")
        .await?;
    client
        .execute(&stmt, &[&account_id])
        .await
        .map_err(ApiError::DeleteAccountError)?;
    Ok(())
}

pub async fn create_account_session(
    client: &Client,
    account_id: i64,
    token_hash: &str,
    expires_at: &DateTime<Utc>,
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached(
            "INSERT INTO groupscape.account_sessions (account_id, token_hash, expires_at) VALUES ($1, $2, $3)",
        )
        .await?;
    client
        .execute(&stmt, &[&account_id, &token_hash, expires_at])
        .await
        .map_err(ApiError::CreateAccountSessionError)?;
    Ok(())
}

pub async fn get_account_by_session_token_hash(
    client: &Client,
    token_hash: &str,
) -> Result<Option<crate::models::Account>, ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
SELECT a.id, a.email, a.created_at
FROM groupscape.account_sessions s
INNER JOIN groupscape.accounts a ON a.id = s.account_id
WHERE s.token_hash = $1 AND s.expires_at > NOW() AND a.disabled = false
"#,
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&token_hash])
        .await
        .map_err(ApiError::GetAccountError)?;
    match row {
        Some(row) => Ok(Some(crate::models::Account {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            created_at: row.try_get("created_at")?,
        })),
        None => Ok(None),
    }
}

/// Ported from `groupscape-old`'s `characters` repository - a per-account cap keeps one
/// account from squatting on an unbounded number of RuneScape accounts.
pub const CHARACTER_CAP_PER_ACCOUNT: i64 = 5;

pub struct Character {
    pub id: i64,
    pub account_id: i64,
    pub account_hash: String,
    pub display_rsn: String,
    pub bound_at: DateTime<Utc>,
}

pub async fn find_character_by_account_hash(
    client: &Client,
    account_hash: &str,
) -> Result<Option<Character>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT character_id, account_id, account_hash, display_rsn, bound_at FROM groupscape.characters WHERE account_hash=$1",
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&account_hash])
        .await
        .map_err(ApiError::GetCharacterError)?;
    match row {
        Some(row) => Ok(Some(Character {
            id: row.try_get("character_id")?,
            account_id: row.try_get("account_id")?,
            account_hash: row.try_get("account_hash")?,
            display_rsn: row.try_get("display_rsn")?,
            bound_at: row.try_get("bound_at")?,
        })),
        None => Ok(None),
    }
}

pub async fn count_characters_for_account(
    client: &Client,
    account_id: i64,
) -> Result<i64, ApiError> {
    let stmt = client
        .prepare_cached("SELECT COUNT(*) FROM groupscape.characters WHERE account_id=$1")
        .await?;
    let row = client
        .query_one(&stmt, &[&account_id])
        .await
        .map_err(ApiError::GetCharacterError)?;
    Ok(row.try_get(0)?)
}

pub async fn create_character(
    client: &Client,
    account_id: i64,
    account_hash: &str,
    display_rsn: &str,
) -> Result<Character, ApiError> {
    let stmt = client
        .prepare_cached(
            "INSERT INTO groupscape.characters (account_id, account_hash, display_rsn) VALUES ($1, $2, $3) RETURNING character_id, account_id, account_hash, display_rsn, bound_at",
        )
        .await?;
    let row = client
        .query_one(&stmt, &[&account_id, &account_hash, &display_rsn])
        .await
        .map_err(ApiError::CreateCharacterError)?;
    Ok(Character {
        id: row.try_get("character_id")?,
        account_id: row.try_get("account_id")?,
        account_hash: row.try_get("account_hash")?,
        display_rsn: row.try_get("display_rsn")?,
        bound_at: row.try_get("bound_at")?,
    })
}

pub async fn update_character_display_rsn(
    client: &Client,
    character_id: i64,
    display_rsn: &str,
) -> Result<Character, ApiError> {
    let stmt = client
        .prepare_cached(
            "UPDATE groupscape.characters SET display_rsn=$1 WHERE character_id=$2 RETURNING character_id, account_id, account_hash, display_rsn, bound_at",
        )
        .await?;
    let row = client
        .query_one(&stmt, &[&display_rsn, &character_id])
        .await
        .map_err(ApiError::GetCharacterError)?;
    Ok(Character {
        id: row.try_get("character_id")?,
        account_id: row.try_get("account_id")?,
        account_hash: row.try_get("account_hash")?,
        display_rsn: row.try_get("display_rsn")?,
        bound_at: row.try_get("bound_at")?,
    })
}

pub async fn find_character_by_id(
    client: &Client,
    character_id: i64,
) -> Result<Option<Character>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT character_id, account_id, account_hash, display_rsn, bound_at FROM groupscape.characters WHERE character_id=$1",
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&character_id])
        .await
        .map_err(ApiError::GetCharacterError)?;
    match row {
        Some(row) => Ok(Some(Character {
            id: row.try_get("character_id")?,
            account_id: row.try_get("account_id")?,
            account_hash: row.try_get("account_hash")?,
            display_rsn: row.try_get("display_rsn")?,
            bound_at: row.try_get("bound_at")?,
        })),
        None => Ok(None),
    }
}

pub async fn list_characters_for_account(
    client: &Client,
    account_id: i64,
) -> Result<Vec<Character>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT character_id, account_id, account_hash, display_rsn, bound_at FROM groupscape.characters WHERE account_id=$1 ORDER BY bound_at ASC",
        )
        .await?;
    let rows = client
        .query(&stmt, &[&account_id])
        .await
        .map_err(ApiError::GetCharacterError)?;
    rows.into_iter()
        .map(|row| {
            Ok(Character {
                id: row.try_get("character_id")?,
                account_id: row.try_get("account_id")?,
                account_hash: row.try_get("account_hash")?,
                display_rsn: row.try_get("display_rsn")?,
                bound_at: row.try_get("bound_at")?,
            })
        })
        .collect()
}

/// Unlinks a character from its account. `character_group_links` references `character_id`
/// with `ON DELETE CASCADE`, so this also drops any group membership the character held -
/// unlike `groupscape-old`, which lacked that cascade and had to delete the link row itself
/// first.
pub async fn delete_character(client: &Client, character_id: i64) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached("DELETE FROM groupscape.characters WHERE character_id=$1")
        .await?;
    client
        .execute(&stmt, &[&character_id])
        .await
        .map_err(ApiError::DeleteCharacterError)?;
    Ok(())
}

pub struct CharacterGroupLink {
    pub character_id: i64,
    pub group_id: i64,
    pub linked_at: DateTime<Utc>,
}

pub async fn find_character_group_link(
    client: &Client,
    character_id: i64,
) -> Result<Option<CharacterGroupLink>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT character_id, group_id, linked_at FROM groupscape.character_group_links WHERE character_id=$1",
        )
        .await?;
    let row = client
        .query_opt(&stmt, &[&character_id])
        .await
        .map_err(ApiError::GetCharacterGroupLinkError)?;
    match row {
        Some(row) => Ok(Some(CharacterGroupLink {
            character_id: row.try_get("character_id")?,
            group_id: row.try_get("group_id")?,
            linked_at: row.try_get("linked_at")?,
        })),
        None => Ok(None),
    }
}

/// Enforces one-group-per-character: `character_id` is the table's primary key, so a
/// character can hold at most one link row. Re-linking to the same group is an idempotent
/// no-op (returns the existing row); linking to a different group is a conflict the caller
/// must resolve by leaving the current group first - ported from `groupscape-old`'s
/// `character_group_links` invariant (PK on `character_id`).
///
/// The account that owns the first character ever linked into a group becomes that group's
/// admin (`groups.admin_account_id`, set once via `WHERE admin_account_id IS NULL` and left
/// alone afterward). `groupscape-old` sets `owner_account_id` at group *creation* instead,
/// since its `create_group` route already requires a logged-in account; this codebase's
/// `unauthed::create_group` predates accounts and stays fully anonymous (ported as-is, out of
/// this ticket's scope), so linking - the only point an authenticated account ever meets a
/// group - is where "first user" is actually observable.
pub async fn link_character_to_group(
    client: &mut Client,
    character_id: i64,
    account_id: i64,
    group_id: i64,
) -> Result<CharacterGroupLink, ApiError> {
    if let Some(existing) = find_character_group_link(client, character_id).await? {
        if existing.group_id != group_id {
            return Err(ApiError::CharacterAlreadyInGroupError);
        }
        return Ok(existing);
    }

    let transaction = client.transaction().await?;

    let insert_stmt = transaction
        .prepare_cached(
            "INSERT INTO groupscape.character_group_links (character_id, group_id) VALUES ($1, $2) RETURNING character_id, group_id, linked_at",
        )
        .await?;
    let row = transaction
        .query_one(&insert_stmt, &[&character_id, &group_id])
        .await
        .map_err(ApiError::LinkCharacterToGroupError)?;

    let admin_stmt = transaction
        .prepare_cached(
            "UPDATE groupscape.groups SET admin_account_id=$1 WHERE group_id=$2 AND admin_account_id IS NULL",
        )
        .await?;
    transaction
        .execute(&admin_stmt, &[&account_id, &group_id])
        .await
        .map_err(ApiError::LinkCharacterToGroupError)?;

    // New member default: every flag off (ported from `groupscape-old`'s `createMembership`,
    // §6) - the group admin's implicit all-permissions is computed from
    // `groups.admin_account_id` at read time, not stored as a row here.
    let permissions_stmt = transaction
        .prepare_cached(
            "INSERT INTO groupscape.group_permissions (group_id, account_id) VALUES ($1, $2) ON CONFLICT (group_id, account_id) DO NOTHING",
        )
        .await?;
    transaction
        .execute(&permissions_stmt, &[&group_id, &account_id])
        .await
        .map_err(ApiError::LinkCharacterToGroupError)?;

    transaction
        .commit()
        .await
        .map_err(ApiError::LinkCharacterToGroupError)?;

    Ok(CharacterGroupLink {
        character_id: row.try_get("character_id")?,
        group_id: row.try_get("group_id")?,
        linked_at: row.try_get("linked_at")?,
    })
}

/// The account of the first character ever linked into `group_id`, or `None` for a group
/// nobody with an account has joined yet. Feeds "admin has all permissions by default"
/// (the permission model this ticket unblocks).
pub async fn get_group_admin_account_id(
    client: &Client,
    group_id: i64,
) -> Result<Option<i64>, ApiError> {
    let stmt = client
        .prepare_cached("SELECT admin_account_id FROM groupscape.groups WHERE group_id=$1")
        .await?;
    let row = client
        .query_opt(&stmt, &[&group_id])
        .await
        .map_err(ApiError::GetGroupAdminError)?;
    match row {
        Some(row) => Ok(row.try_get("admin_account_id")?),
        None => Ok(None),
    }
}

const GROUP_PERMISSION_COLUMNS: &str = "invite_members, regenerate_group_key, kick_members, manage_settings, manage_permissions, post_map_markers, post_callouts, manage_goals, manage_discord, manage_events";

fn group_permissions_from_row(row: &tokio_postgres::Row) -> Result<GroupPermissions, ApiError> {
    Ok(GroupPermissions {
        group_id: row.try_get("group_id")?,
        account_id: row.try_get("account_id")?,
        flags: PermissionFlags {
            invite_members: row.try_get("invite_members")?,
            regenerate_group_key: row.try_get("regenerate_group_key")?,
            kick_members: row.try_get("kick_members")?,
            manage_settings: row.try_get("manage_settings")?,
            manage_permissions: row.try_get("manage_permissions")?,
            post_map_markers: row.try_get("post_map_markers")?,
            post_callouts: row.try_get("post_callouts")?,
            manage_goals: row.try_get("manage_goals")?,
            manage_discord: row.try_get("manage_discord")?,
            manage_events: row.try_get("manage_events")?,
        },
    })
}

/// `None` when the account has no permissions row for this group (never joined via
/// `link_character_to_group`) - callers that need a default-false view for a group member
/// should treat that the same as [`PermissionFlags::default`].
pub async fn get_group_permissions(
    client: &Client,
    group_id: i64,
    account_id: i64,
) -> Result<Option<GroupPermissions>, ApiError> {
    let stmt = client
        .prepare_cached(&format!(
            "SELECT group_id, account_id, {GROUP_PERMISSION_COLUMNS} FROM groupscape.group_permissions WHERE group_id=$1 AND account_id=$2"
        ))
        .await?;
    let row = client
        .query_opt(&stmt, &[&group_id, &account_id])
        .await
        .map_err(ApiError::GetGroupPermissionsError)?;
    row.as_ref().map(group_permissions_from_row).transpose()
}

pub async fn list_group_permissions(
    client: &Client,
    group_id: i64,
) -> Result<Vec<GroupPermissions>, ApiError> {
    let stmt = client
        .prepare_cached(&format!(
            "SELECT group_id, account_id, {GROUP_PERMISSION_COLUMNS} FROM groupscape.group_permissions WHERE group_id=$1"
        ))
        .await?;
    let rows = client
        .query(&stmt, &[&group_id])
        .await
        .map_err(ApiError::GetGroupPermissionsError)?;
    rows.iter().map(group_permissions_from_row).collect()
}

/// One row per account currently linked into `group_id`, joined to that account's most
/// recently bound character for a display name - feeds the permission-management UI, which
/// needs to show *who* a toggle belongs to, not just an opaque `account_id`. `DISTINCT ON`
/// collapses an account with multiple characters in the group down to one row (permissions are
/// per-account, not per-character).
pub async fn list_group_member_permissions(
    client: &Client,
    group_id: i64,
) -> Result<Vec<GroupMemberPermissions>, ApiError> {
    let admin_account_id = get_group_admin_account_id(client, group_id).await?;
    let stmt = client
        .prepare_cached(&format!(
            "SELECT DISTINCT ON (c.account_id) c.account_id, c.display_rsn, {GROUP_PERMISSION_COLUMNS} \
             FROM groupscape.character_group_links cgl \
             JOIN groupscape.characters c ON c.character_id = cgl.character_id \
             JOIN groupscape.group_permissions gp ON gp.group_id = cgl.group_id AND gp.account_id = c.account_id \
             WHERE cgl.group_id = $1 \
             ORDER BY c.account_id, c.bound_at DESC"
        ))
        .await?;
    let rows = client
        .query(&stmt, &[&group_id])
        .await
        .map_err(ApiError::GetGroupPermissionsError)?;
    rows.iter()
        .map(|row| {
            let account_id: i64 = row.try_get("account_id")?;
            Ok(GroupMemberPermissions {
                account_id,
                display_rsn: row.try_get("display_rsn")?,
                is_admin: admin_account_id == Some(account_id),
                flags: PermissionFlags {
                    invite_members: row.try_get("invite_members")?,
                    regenerate_group_key: row.try_get("regenerate_group_key")?,
                    kick_members: row.try_get("kick_members")?,
                    manage_settings: row.try_get("manage_settings")?,
                    manage_permissions: row.try_get("manage_permissions")?,
                    post_map_markers: row.try_get("post_map_markers")?,
                    post_callouts: row.try_get("post_callouts")?,
                    manage_goals: row.try_get("manage_goals")?,
                    manage_discord: row.try_get("manage_discord")?,
                    manage_events: row.try_get("manage_events")?,
                },
            })
        })
        .collect()
}

/// Partial update - each `None` field leaves its current DB value untouched (COALESCE), same
/// pattern as `admin_set_feature_flag`'s upsert. Returns `None` if the account has no
/// permissions row for this group (not a member).
///
/// The group admin's permission row is never writable through this path - their all-permissions
/// access is implicit (see [`has_group_permission`]) and not stored as toggles, so a patch
/// here could only ever *appear* to demote them while doing nothing (`has_group_permission`
/// would keep overriding it back to `true`). Rejecting the write up front keeps that
/// impossibility visible to callers instead of silently no-op'ing.
pub async fn update_group_permissions(
    client: &Client,
    group_id: i64,
    account_id: i64,
    patch: PermissionFlagsPatch,
) -> Result<Option<GroupPermissions>, ApiError> {
    if get_group_admin_account_id(client, group_id).await? == Some(account_id) {
        return Err(ApiError::CannotModifyGroupAdminPermissionsError);
    }
    let stmt = client
        .prepare_cached(&format!(
            r#"
UPDATE groupscape.group_permissions SET
  invite_members = COALESCE($3, invite_members),
  regenerate_group_key = COALESCE($4, regenerate_group_key),
  kick_members = COALESCE($5, kick_members),
  manage_settings = COALESCE($6, manage_settings),
  manage_permissions = COALESCE($7, manage_permissions),
  post_map_markers = COALESCE($8, post_map_markers),
  post_callouts = COALESCE($9, post_callouts),
  manage_goals = COALESCE($10, manage_goals),
  manage_discord = COALESCE($11, manage_discord),
  manage_events = COALESCE($12, manage_events)
WHERE group_id=$1 AND account_id=$2
RETURNING group_id, account_id, {GROUP_PERMISSION_COLUMNS}
"#
        ))
        .await?;
    let row = client
        .query_opt(
            &stmt,
            &[
                &group_id,
                &account_id,
                &patch.invite_members,
                &patch.regenerate_group_key,
                &patch.kick_members,
                &patch.manage_settings,
                &patch.manage_permissions,
                &patch.post_map_markers,
                &patch.post_callouts,
                &patch.manage_goals,
                &patch.manage_discord,
                &patch.manage_events,
            ],
        )
        .await
        .map_err(ApiError::UpdateGroupPermissionsError)?;
    row.as_ref().map(group_permissions_from_row).transpose()
}

/// Owner is an implicit all-permissions holder, not a toggle-holder (ported from
/// `groupscape-old`'s `hasPermission`, §6) - `account_id` matching `groups.admin_account_id`
/// short-circuits to all-true before any flag is consulted. A member with no permissions row
/// (never linked into the group) falls back to [`PermissionFlags::default`], i.e. every flag
/// off.
pub async fn get_effective_permission_flags(
    client: &Client,
    group_id: i64,
    account_id: i64,
) -> Result<PermissionFlags, ApiError> {
    if get_group_admin_account_id(client, group_id).await? == Some(account_id) {
        return Ok(PermissionFlags::all_true());
    }
    let flags = get_group_permissions(client, group_id, account_id)
        .await?
        .map(|permissions| permissions.flags)
        .unwrap_or_default();
    Ok(flags)
}

pub async fn has_group_permission(
    client: &Client,
    group_id: i64,
    account_id: i64,
    key: PermissionKey,
) -> Result<bool, ApiError> {
    let flags = get_effective_permission_flags(client, group_id, account_id).await?;
    Ok(flags.get(key))
}

/// Find-or-create-and-refresh the group's currently open session in one atomic upsert (the
/// partial unique index on `(group_id) WHERE ended_at IS NULL` is what makes this race-safe
/// under concurrent heartbeats from multiple group members) - mirrors `groupscape-old`'s
/// `ensureOpenSession`, called on every heartbeat that carries at least one event.
pub async fn ensure_open_session(client: &Client, group_id: i64) -> Result<i64, ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO groupscape.sessions (group_id) VALUES ($1)
ON CONFLICT (group_id) WHERE ended_at IS NULL
DO UPDATE SET last_seen_at = now()
RETURNING session_id
"#,
        )
        .await?;
    let row = client
        .query_one(&stmt, &[&group_id])
        .await
        .map_err(ApiError::EnsureOpenSessionError)?;
    Ok(row.try_get("session_id")?)
}

/// Closes every session that's gone quiet for `idle_after`, stamping `ended_at` with the
/// session's own `last_seen_at` (the time of its last heartbeat) rather than "now" - mirrors
/// `groupscape-old`'s `closeIdleSessions` recurring job. Global sweep, not scoped to one
/// group, since it's meant to run periodically across every group.
pub async fn close_idle_sessions(
    client: &Client,
    idle_after: chrono::Duration,
) -> Result<u64, ApiError> {
    let stmt = client
        .prepare_cached(
            "UPDATE groupscape.sessions SET ended_at = last_seen_at WHERE ended_at IS NULL AND last_seen_at < $1",
        )
        .await?;
    let cutoff = Utc::now() - idle_after;
    let rows_affected = client
        .execute(&stmt, &[&cutoff])
        .await
        .map_err(ApiError::CloseIdleSessionsError)?;
    Ok(rows_affected)
}

/// Shared insert behind [`insert_activity_event`]/[`insert_dialogue_event`]/
/// [`insert_object_interaction_event`] - `groupscape.activity_events`' `(event_type, payload)`
/// columns are already generic enough to hold any discrete event kind, so NPC
/// dialogue/object-interaction events reuse this table and its `GET /get-activity-events`
/// endpoint rather than getting a dedicated table.
async fn insert_activity_event_payload(
    client: &Client,
    group_id: i64,
    session_id: i64,
    member_name: &str,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached(
            "INSERT INTO groupscape.activity_events (session_id, group_id, member_name, event_type, payload) VALUES ($1, $2, $3, $4, $5)",
        )
        .await?;
    client
        .execute(
            &stmt,
            &[&session_id, &group_id, &member_name, &event_type, &payload],
        )
        .await
        .map_err(ApiError::InsertActivityEventError)?;
    Ok(())
}

/// Stores one discrete kill/death event. Scoped by `member_name` (the roster identity used
/// throughout this server) rather than `groupscape-old`'s `actor_character_ids` co-attribution
/// array - this server doesn't track other members' live world/position with enough recency to
/// attribute a kill to more than the reporting member, so multi-actor credit is left for a
/// follow-up rather than modeled here.
pub async fn insert_activity_event(
    client: &Client,
    group_id: i64,
    session_id: i64,
    member_name: &str,
    event: &GameEvent,
) -> Result<(), ApiError> {
    let payload = serde_json::to_value(event).map_err(ApiError::SerdeJsonError)?;
    insert_activity_event_payload(
        client,
        group_id,
        session_id,
        member_name,
        event.event_type(),
        payload,
    )
    .await
}

/// Stores one NPC dialogue event from the plugin's "interactions" upload key.
pub async fn insert_dialogue_event(
    client: &Client,
    group_id: i64,
    session_id: i64,
    member_name: &str,
    event: &DialogueEvent,
) -> Result<(), ApiError> {
    let payload = serde_json::to_value(event).map_err(ApiError::SerdeJsonError)?;
    insert_activity_event_payload(
        client,
        group_id,
        session_id,
        member_name,
        "dialogue",
        payload,
    )
    .await
}

/// Stores one object-interaction event from the plugin's "object_interactions" upload key.
pub async fn insert_object_interaction_event(
    client: &Client,
    group_id: i64,
    session_id: i64,
    member_name: &str,
    event: &ObjectInteractionEvent,
) -> Result<(), ApiError> {
    let payload = serde_json::to_value(event).map_err(ApiError::SerdeJsonError)?;
    insert_activity_event_payload(
        client,
        group_id,
        session_id,
        member_name,
        "object_interaction",
        payload,
    )
    .await
}

fn activity_event_from_row(row: &Row) -> Result<ActivityEvent, ApiError> {
    Ok(ActivityEvent {
        id: row.try_get("event_id")?,
        session_id: row.try_get("session_id")?,
        member_name: row.try_get("member_name")?,
        event_type: row.try_get("event_type")?,
        occurred_at: row.try_get("occurred_at")?,
        payload: row.try_get("payload")?,
    })
}

/// Paginated, newest-first activity feed for a group, optionally filtered by member and/or
/// event type, with a `before` cursor (mirrors `groupscape-old`'s `GET /groups/:id/activity`).
#[allow(clippy::too_many_arguments)]
pub async fn list_activity_events(
    client: &Client,
    group_id: i64,
    member_name: Option<&str>,
    event_type: Option<&str>,
    before: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<ActivityEvent>, ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
SELECT event_id, session_id, member_name, event_type, occurred_at, payload
FROM groupscape.activity_events
WHERE group_id=$1
  AND ($2::text IS NULL OR member_name = $2)
  AND ($3::text IS NULL OR event_type = $3)
  AND ($4::timestamptz IS NULL OR occurred_at < $4)
ORDER BY occurred_at DESC
LIMIT $5
"#,
        )
        .await?;
    let rows = client
        .query(
            &stmt,
            &[
                &group_id,
                &member_name,
                &event_type,
                &before,
                &limit.clamp(1, 200),
            ],
        )
        .await
        .map_err(ApiError::ListActivityEventsError)?;
    rows.iter().map(activity_event_from_row).collect()
}

/// All `kill` events for a group in an optional `[since, until]` range, uncapped by cursor
/// pagination (bounded by a hard safety limit) since callers aggregate the full result rather
/// than paging it - mirrors `groupscape-old`'s `listBossKillEventsForGroup` query-time pivot
/// that the loot summary/split endpoints are built on.
pub async fn list_kill_events(
    client: &Client,
    group_id: i64,
    member_name: Option<&str>,
    since: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
) -> Result<Vec<ActivityEvent>, ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
SELECT event_id, session_id, member_name, event_type, occurred_at, payload
FROM groupscape.activity_events
WHERE group_id=$1
  AND event_type='kill'
  AND ($2::text IS NULL OR member_name = $2)
  AND ($3::timestamptz IS NULL OR occurred_at >= $3)
  AND ($4::timestamptz IS NULL OR occurred_at <= $4)
ORDER BY occurred_at DESC
LIMIT 5000
"#,
        )
        .await?;
    let rows = client
        .query(&stmt, &[&group_id, &member_name, &since, &until])
        .await
        .map_err(ApiError::ListKillEventsError)?;
    rows.iter().map(activity_event_from_row).collect()
}

fn group_session_from_row(row: &Row) -> Result<GroupSession, ApiError> {
    Ok(GroupSession {
        id: row.try_get("session_id")?,
        started_at: row.try_get("started_at")?,
        ended_at: row.try_get("ended_at")?,
    })
}

/// Newest-first session list for a group (open session, if any, sorts first since it has no
/// `ended_at`... actually ordered by `started_at DESC` so it's always first regardless).
pub async fn list_sessions(
    client: &Client,
    group_id: i64,
    limit: i64,
) -> Result<Vec<GroupSession>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT session_id, started_at, ended_at FROM groupscape.sessions WHERE group_id=$1 ORDER BY started_at DESC LIMIT $2",
        )
        .await?;
    let rows = client
        .query(&stmt, &[&group_id, &limit.clamp(1, 200)])
        .await
        .map_err(ApiError::ListSessionsError)?;
    rows.iter().map(group_session_from_row).collect()
}

pub async fn admin_record_audit_log(
    client: &Client,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<&str>,
    detail: Option<serde_json::Value>,
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached(
            "INSERT INTO groupscape.admin_audit_log (action, target_type, target_id, detail) VALUES ($1, $2, $3, $4)",
        )
        .await?;
    client
        .execute(&stmt, &[&action, &target_type, &target_id, &detail])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminRecordAuditLogError".to_string(), e))?;
    Ok(())
}

pub async fn admin_list_groups(
    client: &Client,
    search: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<(Vec<AdminGroupSummary>, i64), ApiError> {
    let offset = (page.max(1) - 1) * page_size.max(1);
    let search_pattern = search.map(|s| format!("%{}%", s));

    let list_stmt = client
        .prepare_cached(
            r#"
SELECT g.group_id, g.group_name, g.version,
  (SELECT COUNT(*) FROM groupscape.members m WHERE m.group_id = g.group_id AND m.member_name != $1) AS member_count,
  COALESCE(gm.status, 'active') AS status
FROM groupscape.groups g
LEFT JOIN groupscape.group_moderation gm ON gm.group_id = g.group_id
WHERE $2::text IS NULL OR g.group_name ILIKE $2
ORDER BY g.group_id DESC
LIMIT $3 OFFSET $4
"#,
        )
        .await?;
    let rows = client
        .query(
            &list_stmt,
            &[&SHARED_MEMBER, &search_pattern, &page_size.max(1), &offset],
        )
        .await
        .map_err(|e| ApiError::AdminDbError("AdminListGroupsError".to_string(), e))?;

    let groups = rows
        .into_iter()
        .map(|row| AdminGroupSummary {
            group_id: row.get("group_id"),
            group_name: row.get("group_name"),
            version: row.get("version"),
            member_count: row.get("member_count"),
            status: row.get("status"),
        })
        .collect();

    let count_stmt = client
        .prepare_cached(
            "SELECT COUNT(*) FROM groupscape.groups g WHERE $1::text IS NULL OR g.group_name ILIKE $1",
        )
        .await?;
    let total: i64 = client
        .query_one(&count_stmt, &[&search_pattern])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminCountGroupsError".to_string(), e))?
        .try_get(0)?;

    Ok((groups, total))
}

pub async fn admin_get_group(
    client: &Client,
    group_id: i64,
) -> Result<Option<AdminGroupDetail>, ApiError> {
    let group_stmt = client
        .prepare_cached(
            r#"
SELECT g.group_id, g.group_name, g.version, COALESCE(gm.status, 'active') AS status, gm.reason
FROM groupscape.groups g
LEFT JOIN groupscape.group_moderation gm ON gm.group_id = g.group_id
WHERE g.group_id = $1
"#,
        )
        .await?;
    let group_row = client
        .query_opt(&group_stmt, &[&group_id])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminGetGroupError".to_string(), e))?;
    let Some(group_row) = group_row else {
        return Ok(None);
    };

    let members_stmt = client
        .prepare_cached(
            "SELECT member_name FROM groupscape.members WHERE group_id = $1 AND member_name != $2 ORDER BY member_name",
        )
        .await?;
    let member_rows = client
        .query(&members_stmt, &[&group_id, &SHARED_MEMBER])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminGetGroupMembersError".to_string(), e))?;
    let members = member_rows.into_iter().map(|row| row.get(0)).collect();

    Ok(Some(AdminGroupDetail {
        group_id: group_row.get("group_id"),
        group_name: group_row.get("group_name"),
        version: group_row.get("version"),
        status: group_row.get("status"),
        reason: group_row.get("reason"),
        members,
    }))
}

pub async fn admin_set_group_moderation(
    client: &Client,
    group_id: i64,
    status: &str,
    reason: Option<&str>,
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO groupscape.group_moderation (group_id, status, reason, updated_at)
VALUES ($1, $2, $3, now())
ON CONFLICT (group_id) DO UPDATE SET status = $2, reason = $3, updated_at = now()
"#,
        )
        .await?;
    client
        .execute(&stmt, &[&group_id, &status, &reason])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminSetGroupModerationError".to_string(), e))?;
    Ok(())
}

pub async fn admin_delete_group(client: &mut Client, group_id: i64) -> Result<(), ApiError> {
    let member_id_stmt = client
        .prepare_cached("SELECT member_id FROM groupscape.members WHERE group_id = $1")
        .await?;
    let member_ids: Vec<i64> = client
        .query(&member_id_stmt, &[&group_id])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminDeleteGroupMembersLookupError".to_string(), e))?
        .into_iter()
        .map(|row| row.get(0))
        .collect();

    let transaction = client.transaction().await?;

    for member_id in member_ids {
        delete_skills_data_for_member(&transaction, AggregatePeriod::Day, member_id).await?;
        delete_skills_data_for_member(&transaction, AggregatePeriod::Month, member_id).await?;
        delete_skills_data_for_member(&transaction, AggregatePeriod::Year, member_id).await?;
        delete_collection_log_data_for_member(&transaction, member_id).await?;
    }

    let delete_members_stmt = transaction
        .prepare_cached("DELETE FROM groupscape.members WHERE group_id = $1")
        .await?;
    transaction
        .execute(&delete_members_stmt, &[&group_id])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminDeleteGroupMembersError".to_string(), e))?;

    let delete_moderation_stmt = transaction
        .prepare_cached("DELETE FROM groupscape.group_moderation WHERE group_id = $1")
        .await?;
    transaction
        .execute(&delete_moderation_stmt, &[&group_id])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminDeleteGroupModerationError".to_string(), e))?;

    let delete_group_stmt = transaction
        .prepare_cached("DELETE FROM groupscape.groups WHERE group_id = $1")
        .await?;
    transaction
        .execute(&delete_group_stmt, &[&group_id])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminDeleteGroupError".to_string(), e))?;

    transaction
        .commit()
        .await
        .map_err(|e| ApiError::AdminDbError("AdminDeleteGroupCommitError".to_string(), e))?;

    Ok(())
}

pub async fn admin_list_feature_flags(client: &Client) -> Result<Vec<AdminFeatureFlag>, ApiError> {
    let stmt = client
        .prepare_cached(
            "SELECT flag_key, enabled, description FROM groupscape.feature_flags ORDER BY flag_key",
        )
        .await?;
    let rows = client
        .query(&stmt, &[])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminListFeatureFlagsError".to_string(), e))?;
    Ok(rows
        .into_iter()
        .map(|row| AdminFeatureFlag {
            flag_key: row.get("flag_key"),
            enabled: row.get("enabled"),
            description: row.get("description"),
        })
        .collect())
}

pub async fn admin_set_feature_flag(
    client: &Client,
    flag_key: &str,
    enabled: bool,
    description: Option<&str>,
) -> Result<(), ApiError> {
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO groupscape.feature_flags (flag_key, enabled, description, updated_at)
VALUES ($1, $2, $3, now())
ON CONFLICT (flag_key) DO UPDATE SET enabled = $2, description = COALESCE($3, groupscape.feature_flags.description), updated_at = now()
"#,
        )
        .await?;
    client
        .execute(&stmt, &[&flag_key, &enabled, &description])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminSetFeatureFlagError".to_string(), e))?;
    Ok(())
}

pub async fn admin_list_audit_log(
    client: &Client,
    page: i64,
    page_size: i64,
) -> Result<(Vec<AdminAuditLogEntry>, i64), ApiError> {
    let offset = (page.max(1) - 1) * page_size.max(1);

    let list_stmt = client
        .prepare_cached(
            r#"
SELECT id, action, target_type, target_id, detail, created_at
FROM groupscape.admin_audit_log
ORDER BY created_at DESC
LIMIT $1 OFFSET $2
"#,
        )
        .await?;
    let rows = client
        .query(&list_stmt, &[&page_size.max(1), &offset])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminListAuditLogError".to_string(), e))?;
    let entries = rows
        .into_iter()
        .map(|row| AdminAuditLogEntry {
            id: row.get("id"),
            action: row.get("action"),
            target_type: row.get("target_type"),
            target_id: row.get("target_id"),
            detail: row.get("detail"),
            created_at: row.get("created_at"),
        })
        .collect();

    let total: i64 = client
        .query_one("SELECT COUNT(*) FROM groupscape.admin_audit_log", &[])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminCountAuditLogError".to_string(), e))?
        .try_get(0)?;

    Ok((entries, total))
}

pub async fn admin_count_accounts(client: &Client) -> Result<i64, ApiError> {
    let count: i64 = client
        .query_one("SELECT COUNT(*) FROM groupscape.accounts", &[])
        .await
        .map_err(|e| ApiError::AdminDbError("AdminCountAccountsError".to_string(), e))?
        .try_get(0)?;
    Ok(count)
}
