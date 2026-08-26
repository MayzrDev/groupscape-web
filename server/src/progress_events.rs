//! Diffs a member's previously-stored progress snapshot against the one riding the current
//! heartbeat and turns the transitions into persisted activity-feed events.
//!
//! Only the milestones the activity feed is scoped to are produced here: quest completions,
//! achievement-diary tier completions, individual combat-achievement task completions, and
//! collection-log item additions / page completions. Kills and deaths come off the plugin's own
//! discrete `events` key instead (see [`crate::models::GameEvent`]).
//!
//! This has to run synchronously in the update handler rather than in
//! [`crate::update_batcher`] - the batcher only does latest-value-wins upserts and never reads the
//! old row, so there is nothing to diff against there.

use crate::collection_log_content;
use crate::diary_tiers;
use crate::models::{CombatAchievements, GroupMember};
use crate::quest_ids::{self, QUEST_STATE_FINISHED};
use serde_json::json;
use std::collections::HashMap;

pub const EVENT_TYPE_QUEST: &str = "quest";
pub const EVENT_TYPE_DIARY: &str = "diary";
pub const EVENT_TYPE_COMBAT_TASK: &str = "combat_task";
pub const EVENT_TYPE_COLLECTION_LOG: &str = "collection_log";

/// The member's previously-stored progress columns, read straight from `groupscape.members`.
/// A `None` field means "never populated" - those are never diffed, so a member's first-ever
/// upload doesn't replay their whole existing account as freshly-completed (mirrors the
/// `if (!previousQuests) return;` guard in `site/src/data/member-data.js`).
#[derive(Default)]
pub struct ProgressSnapshot {
    pub quests: Option<Vec<u8>>,
    pub diary_vars: Option<Vec<i32>>,
    pub collection_log: Option<Vec<i32>>,
    pub combat_achievements: Option<CombatAchievements>,
}

pub struct ProgressEvent {
    pub event_type: &'static str,
    pub payload: serde_json::Value,
}

/// Quest ids whose stored state transitioned into FINISHED.
///
/// Payload is just `{ "quest_id": <id> }` - the frontend already has `Quest.questData` loaded and
/// resolves the name/difficulty from the id, so there's no reason to duplicate that here.
fn diff_quests(previous: &[u8], current: &[u8]) -> Vec<ProgressEvent> {
    let mut events = Vec::new();
    for (index, state) in current.iter().enumerate() {
        if *state != QUEST_STATE_FINISHED {
            continue;
        }
        if previous.get(index) == Some(&QUEST_STATE_FINISHED) {
            continue;
        }
        // A quest index the previous upload didn't even have (shorter array) still counts as a
        // transition - the stored state for it was not FINISHED.
        let Some(quest_id) = quest_ids::quest_id_at(index) else {
            continue;
        };
        events.push(ProgressEvent {
            event_type: EVENT_TYPE_QUEST,
            payload: json!({ "quest_id": quest_id }),
        });
    }
    events
}

/// Region+tier pairs that are fully complete now but weren't before.
fn diff_diaries(previous: &[i32], current: &[i32]) -> Vec<ProgressEvent> {
    let previous_tiers = diary_tiers::parse_diary_data(previous);
    let current_tiers = diary_tiers::parse_diary_data(current);

    current_tiers
        .iter()
        .zip(previous_tiers.iter())
        .filter(|(current_tier, previous_tier)| {
            current_tier.is_complete() && !previous_tier.is_complete()
        })
        .map(|(current_tier, _)| ProgressEvent {
            event_type: EVENT_TYPE_DIARY,
            payload: json!({ "region": current_tier.region, "tier": current_tier.tier }),
        })
        .collect()
}

/// Task ids newly present in the plugin's sparse `tasks` map. The plugin only ever includes
/// completed (`true`) entries and re-sends the whole snapshot each upload, so a key appearing for
/// the first time is a completion.
fn diff_combat_tasks(
    previous: &CombatAchievements,
    current: &CombatAchievements,
) -> Vec<ProgressEvent> {
    let mut task_ids: Vec<i64> = current
        .tasks
        .iter()
        .filter(|(task_id, complete)| **complete && !previous.tasks.contains_key(*task_id))
        .filter_map(|(task_id, _)| task_id.parse::<i64>().ok())
        .collect();
    // HashMap iteration order isn't stable; sort so a multi-task upload persists deterministically.
    task_ids.sort_unstable();

    task_ids
        .into_iter()
        .map(|task_id| ProgressEvent {
            event_type: EVENT_TYPE_COMBAT_TASK,
            payload: json!({ "task_id": task_id }),
        })
        .collect()
}

/// Flat `[item_id, quantity, ...]` pairs -> canonical item id -> total quantity.
fn collection_log_quantities(flat: &[i32]) -> HashMap<i32, i32> {
    let mut quantities = HashMap::new();
    for pair in flat.chunks_exact(2) {
        let item_id = collection_log_content::canonical_item_id(pair[0]);
        if item_id <= 0 {
            continue;
        }
        *quantities.entry(item_id).or_insert(0) += pair[1];
    }
    quantities
}

fn page_is_complete(items: &[i32], quantities: &HashMap<i32, i32>) -> bool {
    items
        .iter()
        .all(|item_id| quantities.get(item_id).copied().unwrap_or(0) > 0)
}

/// Newly-obtained collection-log items, plus any page those additions just completed. Both ride
/// the single `collection_log` event type, discriminated by the payload's `kind`, because the
/// feed filter chip and the toast treat them as one "Collection log" bucket.
fn diff_collection_log(previous: &[i32], current: &[i32]) -> Vec<ProgressEvent> {
    let previous_quantities = collection_log_quantities(previous);
    let current_quantities = collection_log_quantities(current);

    let mut added: Vec<(i32, i32)> = current_quantities
        .iter()
        .filter(|(item_id, quantity)| {
            **quantity > 0 && previous_quantities.get(*item_id).copied().unwrap_or(0) <= 0
        })
        .map(|(item_id, quantity)| (*item_id, *quantity))
        .collect();
    added.sort_unstable();

    let mut events: Vec<ProgressEvent> = added
        .into_iter()
        .map(|(item_id, quantity)| ProgressEvent {
            event_type: EVENT_TYPE_COLLECTION_LOG,
            payload: json!({ "kind": "item", "item_id": item_id, "quantity": quantity }),
        })
        .collect();

    for (page_name, items) in collection_log_content::pages() {
        if page_is_complete(items, &current_quantities)
            && !page_is_complete(items, &previous_quantities)
        {
            events.push(ProgressEvent {
                event_type: EVENT_TYPE_COLLECTION_LOG,
                payload: json!({ "kind": "page", "page": page_name }),
            });
        }
    }

    events
}

/// Every milestone transition between the stored snapshot and this heartbeat's upload.
///
/// Fields the upload doesn't carry, and fields that were never stored before, are skipped.
pub fn diff_progress(previous: &ProgressSnapshot, current: &GroupMember) -> Vec<ProgressEvent> {
    let mut events = Vec::new();

    if let (Some(previous_quests), Some(current_quests)) = (&previous.quests, &current.quests) {
        events.extend(diff_quests(previous_quests, current_quests));
    }
    if let (Some(previous_vars), Some(current_vars)) = (&previous.diary_vars, &current.diary_vars) {
        events.extend(diff_diaries(previous_vars, current_vars));
    }
    if let (Some(previous_tasks), Some(current_tasks)) =
        (&previous.combat_achievements, &current.combat_achievements)
    {
        events.extend(diff_combat_tasks(previous_tasks, current_tasks));
    }
    if let (Some(previous_log), Some(current_log)) =
        (&previous.collection_log, &current.collection_log_v2)
    {
        events.extend(diff_collection_log(previous_log, current_log));
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combat_achievements(task_ids: &[&str]) -> CombatAchievements {
        CombatAchievements {
            tiers: HashMap::new(),
            tasks: task_ids
                .iter()
                .map(|task_id| (task_id.to_string(), true))
                .collect(),
        }
    }

    #[test]
    fn quest_transition_into_finished_emits_one_event() {
        let mut previous = vec![0u8; 8];
        let mut current = vec![0u8; 8];
        current[2] = QUEST_STATE_FINISHED;
        previous[2] = 1;

        let events = diff_quests(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EVENT_TYPE_QUEST);
        assert_eq!(
            events[0].payload["quest_id"],
            json!(quest_ids::quest_id_at(2).unwrap())
        );
    }

    #[test]
    fn already_finished_quest_emits_nothing() {
        let previous = vec![QUEST_STATE_FINISHED; 4];
        let current = vec![QUEST_STATE_FINISHED; 4];
        assert!(diff_quests(&previous, &current).is_empty());
    }

    #[test]
    fn newly_completed_diary_tier_emits_region_and_tier() {
        let previous = vec![0i32; 62];
        let mut current = vec![0i32; 62];
        // Kandarin Elite is varbit 9 bits 5..=11.
        for bit in 5..=11 {
            current[9] |= 1 << bit;
        }

        let events = diff_diaries(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EVENT_TYPE_DIARY);
        assert_eq!(events[0].payload["region"], json!("Kandarin"));
        assert_eq!(events[0].payload["tier"], json!("Elite"));
    }

    #[test]
    fn unchanged_diary_vars_emit_nothing() {
        let vars = vec![0x0fffi32; 62];
        assert!(diff_diaries(&vars, &vars).is_empty());
    }

    #[test]
    fn newly_present_combat_task_key_emits_one_event() {
        let previous = combat_achievements(&["1", "2"]);
        let current = combat_achievements(&["1", "2", "300"]);

        let events = diff_combat_tasks(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EVENT_TYPE_COMBAT_TASK);
        assert_eq!(events[0].payload["task_id"], json!(300));
    }

    #[test]
    fn resent_combat_task_snapshot_emits_nothing() {
        let snapshot = combat_achievements(&["1", "2"]);
        assert!(diff_combat_tasks(&snapshot, &snapshot).is_empty());
    }

    #[test]
    fn newly_added_collection_log_item_emits_an_item_event() {
        let previous = vec![4151, 1];
        let current = vec![4151, 1, 13262, 1];

        let events = diff_collection_log(&previous, &current);
        let item_events: Vec<_> = events
            .iter()
            .filter(|event| event.payload["kind"] == json!("item"))
            .collect();
        assert_eq!(item_events.len(), 1);
        assert_eq!(item_events[0].event_type, EVENT_TYPE_COLLECTION_LOG);
        assert_eq!(item_events[0].payload["item_id"], json!(13262));
        assert_eq!(item_events[0].payload["quantity"], json!(1));
    }

    #[test]
    fn duplicate_item_ids_collapse_so_they_do_not_double_fire() {
        // 4178 canonicalises onto 4151, so an upload that swaps one for the other is not an add.
        let previous = vec![4151, 1];
        let current = vec![4178, 1];
        assert!(diff_collection_log(&previous, &current).is_empty());
    }

    #[test]
    fn completing_every_item_on_a_page_emits_a_page_event() {
        let (page_name, items) = collection_log_content::pages()
            .iter()
            .find(|(name, _)| name == "Abyssal Sire")
            .expect("Abyssal Sire page must exist");

        let mut current = Vec::new();
        for item_id in items {
            current.push(*item_id);
            current.push(1);
        }
        // One item short beforehand, so the page only completes on this upload.
        let previous = current[..current.len() - 2].to_vec();

        let events = diff_collection_log(&previous, &current);
        let page_events: Vec<_> = events
            .iter()
            .filter(|event| event.payload["kind"] == json!("page"))
            .collect();
        assert_eq!(page_events.len(), 1);
        assert_eq!(page_events[0].payload["page"], json!(page_name));
    }

    #[test]
    fn a_field_never_stored_before_is_not_diffed() {
        let previous = ProgressSnapshot::default();
        let current = GroupMember {
            group_id: Some(1),
            name: "Alice".to_string(),
            account_hash: None,
            color: None,
            stats: None,
            coordinates: None,
            skills: None,
            quests: Some(vec![QUEST_STATE_FINISHED; 8]),
            inventory: None,
            equipment: None,
            bank: None,
            shared_bank: None,
            rune_pouch: None,
            interacting: None,
            seed_vault: None,
            deposited: None,
            diary_vars: Some(vec![0xffffffffu32 as i32; 62]),
            collection_log_v2: Some(vec![4151, 1]),
            potion_storage: None,
            special_attack: None,
            active_prayers: None,
            rich_presence: None,
            combat_achievements: Some(combat_achievements(&["1"])),
            portrait_last_update: None,
            last_updated: None,
            events: None,
            interactions: None,
            object_interactions: None,
            alerts: None,
            notable_drops: None,
        };

        assert!(diff_progress(&previous, &current).is_empty());
    }
}
