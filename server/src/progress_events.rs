//! Diffs a member's previously-stored progress snapshot against the one riding the current
//! heartbeat and turns the transitions into persisted activity-feed events.
//!
//! Only the milestones the activity feed is scoped to are produced here: quest completions,
//! achievement-diary tier completions, individual combat-achievement task completions,
//! collection-log item additions / page completions, and skill level-up milestones. Kills and
//! deaths come off the plugin's own discrete `events` key instead (see
//! [`crate::models::GameEvent`]).
//!
//! This has to run synchronously in the update handler rather than in
//! [`crate::update_batcher`] - the batcher only does latest-value-wins upserts and never reads the
//! old row, so there is nothing to diff against there.

use crate::collection_log_content;
use crate::combat_achievement_content;
use crate::diary_tiers;
use crate::models::{CombatAchievements, GroupMember};
use crate::quest_ids::{self, QUEST_STATE_FINISHED};
use serde_json::json;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const EVENT_TYPE_QUEST: &str = "quest";
pub const EVENT_TYPE_DIARY: &str = "diary";
pub const EVENT_TYPE_COMBAT_TASK: &str = "combat_task";
pub const EVENT_TYPE_COLLECTION_LOG: &str = "collection_log";
pub const EVENT_TYPE_LEVEL_UP: &str = "level_up";

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
    pub skills: Option<Vec<i32>>,
}

#[derive(Clone)]
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

fn task_is_complete(tasks: &HashMap<String, bool>, task_id: i64) -> bool {
    tasks.get(&task_id.to_string()).copied().unwrap_or(false)
}

fn boss_is_complete(tasks: &HashMap<String, bool>, task_ids: &[i64]) -> bool {
    task_ids.iter().all(|task_id| task_is_complete(tasks, *task_id))
}

/// Task ids newly present in the plugin's sparse `tasks` map, plus any boss whose every task just
/// became complete. Both ride the single `combat_task` event type, discriminated by the payload's
/// `kind` - mirrors `diff_collection_log`'s item/page split.
///
/// The plugin only ever includes completed (`true`) entries and re-sends the whole snapshot each
/// upload, so a key appearing for the first time is a completion.
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

    let mut events: Vec<ProgressEvent> = task_ids
        .into_iter()
        .map(|task_id| ProgressEvent {
            event_type: EVENT_TYPE_COMBAT_TASK,
            payload: json!({ "task_id": task_id }),
        })
        .collect();

    for (boss, boss_task_ids) in combat_achievement_content::boss_groups() {
        if boss_is_complete(&current.tasks, boss_task_ids)
            && !boss_is_complete(&previous.tasks, boss_task_ids)
        {
            events.push(ProgressEvent {
                event_type: EVENT_TYPE_COMBAT_TASK,
                payload: json!({ "kind": "boss", "boss": boss }),
            });
        }
    }

    events
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

/// Order matches the stored `skills INTEGER[24]` column (see `db::skill_array_index`'s doc
/// comment) - index 0 is Postgres `skills[1]` (Agility) through index 23, `skills[24]` (Sailing).
/// "Overall" has no slot of its own, so it never posts a level-up milestone.
const SKILL_NAMES: [&str; 24] = [
    "Agility",
    "Attack",
    "Construction",
    "Cooking",
    "Crafting",
    "Defence",
    "Farming",
    "Firemaking",
    "Fishing",
    "Fletching",
    "Herblore",
    "Hitpoints",
    "Hunter",
    "Magic",
    "Mining",
    "Prayer",
    "Ranged",
    "Runecraft",
    "Slayer",
    "Smithing",
    "Strength",
    "Thieving",
    "Woodcutting",
    "Sailing",
];

/// Level milestones the activity feed posts for - every 10 up to 80, every 5 up to 95, then every
/// level up to 99. Common early levels aren't worth a post; the max-level grind is.
const LEVEL_MILESTONES: [u32; 15] = [10, 20, 30, 40, 50, 60, 70, 80, 85, 90, 95, 96, 97, 98, 99];

/// XP required to *start* the given level, ported 1:1 from `site/src/data/skill.js`'s
/// `xpForLevel` so the server's milestone detection agrees with what the client displays.
fn xp_for_level(level: u32) -> u64 {
    let mut xp: f64 = 0.0;
    for i in 1..=level {
        xp += (i as f64 + 300.0 * 2f64.powf(i as f64 / 7.0)).floor();
    }
    (0.25 * xp).floor() as u64
}

/// `table[level]` is the XP required to reach `level` (1-99). Computed once - `xp_for_level`
/// itself is an O(level) sum, and this gets called for 24 skills on every heartbeat.
fn level_xp_table() -> &'static [u64; 100] {
    static TABLE: OnceLock<[u64; 100]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u64; 100];
        for level in 1..=99usize {
            table[level] = xp_for_level(level as u32);
        }
        table
    })
}

/// Levels are capped at 99 here - the milestone schedule never goes past it, so a skill's virtual
/// level beyond 99 (e.g. from bonus XP events) doesn't matter for this purpose.
fn level_for_xp(xp: i32) -> u32 {
    let table = level_xp_table();
    let xp = xp.max(0) as u64;
    (1..=99u32).rev().find(|&level| xp >= table[level as usize]).unwrap_or(1)
}

/// Ceiling on how many `level_up` events a *single heartbeat* can plausibly produce. Legitimate
/// jumps - a big XP-lump reward on one skill, or a raid completion nudging several skills over a
/// threshold at once - stay well under this. A heartbeat that blows past it is a signal that
/// `previous` itself is stale or wrong (e.g. it's carrying an unrelated account's history) rather
/// than that the player genuinely crossed this many thresholds since the last upload - see
/// `authed.rs`'s handling of `diff_progress`, which drops the level-up batch and re-baselines the
/// stored skills snapshot when this is exceeded instead of posting it.
pub const MAX_PLAUSIBLE_LEVEL_UPS_PER_HEARTBEAT: usize = 6;

/// True if `events` contains more `level_up` entries than a single heartbeat could plausibly
/// produce from real play - see `MAX_PLAUSIBLE_LEVEL_UPS_PER_HEARTBEAT`.
pub fn looks_like_stale_skills_baseline(events: &[ProgressEvent]) -> bool {
    events.iter().filter(|event| event.event_type == EVENT_TYPE_LEVEL_UP).count()
        > MAX_PLAUSIBLE_LEVEL_UPS_PER_HEARTBEAT
}

/// Every milestone newly crossed going from `previous_level` to `current_level`, in ascending
/// order. When a heartbeat jumps a skill across more than one threshold at once (e.g. a large
/// XP-lump reward), all of them are backfilled rather than only the highest.
fn milestones_crossed(previous_level: u32, current_level: u32) -> impl Iterator<Item = u32> {
    LEVEL_MILESTONES
        .into_iter()
        .filter(move |&milestone| milestone > previous_level && milestone <= current_level)
}

/// Every individual level a skill crossed this upload, regardless of `LEVEL_MILESTONES` - feeds
/// Discord's own configurable posting interval (`DiscordWebhookSettings::level_up_interval`),
/// which is deliberately decoupled from the activity feed's fixed milestone schedule below. Not
/// persisted to `activity_events` - the caller only uses this to fire Discord dispatches directly,
/// which then filter by the group's interval setting at dispatch time.
///
/// Same "skip a skill slot missing from `previous`" guard as `diff_skills`. Callers are expected to
/// only invoke this when `diff_skills`/`looks_like_stale_skills_baseline` on the same
/// previous/current pair didn't flag the upload as an implausible baseline jump - that check
/// already bounds how many thresholds (and therefore how many individual levels) a legitimate
/// heartbeat can cross.
pub fn diff_skills_fine(previous: &[i32], current: &[i32]) -> Vec<ProgressEvent> {
    let mut events = Vec::new();
    for (index, &current_xp) in current.iter().enumerate() {
        let Some(&previous_xp) = previous.get(index) else {
            continue;
        };
        let Some(&skill) = SKILL_NAMES.get(index) else {
            continue;
        };
        let previous_level = level_for_xp(previous_xp);
        let current_level = level_for_xp(current_xp);
        for level in (previous_level + 1)..=current_level {
            events.push(ProgressEvent {
                event_type: EVENT_TYPE_LEVEL_UP,
                payload: json!({ "skill": skill, "level": level }),
            });
        }
    }
    events
}

/// Skills whose level crossed a new milestone threshold this upload (see `LEVEL_MILESTONES`).
/// A skill slot missing from `previous` (shorter array than `current`) is skipped rather than
/// treated as a level-1 baseline - mirrors the "never diffed before" guard the other diffs use.
fn diff_skills(previous: &[i32], current: &[i32]) -> Vec<ProgressEvent> {
    let mut events = Vec::new();
    for (index, &current_xp) in current.iter().enumerate() {
        let Some(&previous_xp) = previous.get(index) else {
            continue;
        };
        let Some(&skill) = SKILL_NAMES.get(index) else {
            continue;
        };
        let previous_level = level_for_xp(previous_xp);
        let current_level = level_for_xp(current_xp);
        for milestone in milestones_crossed(previous_level, current_level) {
            events.push(ProgressEvent {
                event_type: EVENT_TYPE_LEVEL_UP,
                payload: json!({ "skill": skill, "level": milestone }),
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
    if let (Some(previous_skills), Some(current_skills)) = (&previous.skills, &current.skills) {
        events.extend(diff_skills(previous_skills, current_skills));
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
    fn completing_every_task_for_a_boss_emits_a_boss_event() {
        let (boss, task_ids) = combat_achievement_content::boss_groups()
            .iter()
            .find(|(boss, _)| boss == "Barrows")
            .expect("Barrows boss group must exist");

        let all_ids: Vec<String> = task_ids.iter().map(|id| id.to_string()).collect();
        let current = combat_achievements(&all_ids.iter().map(String::as_str).collect::<Vec<_>>());
        // One task short beforehand, so the boss group only completes on this upload.
        let previous = combat_achievements(
            &all_ids[..all_ids.len() - 1]
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );

        let events = diff_combat_tasks(&previous, &current);
        let boss_events: Vec<_> = events
            .iter()
            .filter(|event| event.payload["kind"] == json!("boss"))
            .collect();
        assert_eq!(boss_events.len(), 1);
        assert_eq!(boss_events[0].payload["boss"], json!(boss));
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
    fn level_for_xp_matches_known_boundaries() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(xp_for_level(2) as i32 - 1), 1);
        assert_eq!(level_for_xp(xp_for_level(2) as i32), 2);
        assert_eq!(level_for_xp(xp_for_level(99) as i32), 99);
    }

    fn skills_xp(agility_xp: i32) -> Vec<i32> {
        let mut skills = vec![0i32; 24];
        skills[0] = agility_xp; // Agility - see SKILL_NAMES.
        skills
    }

    fn all_skills_xp(xp: i32) -> Vec<i32> {
        vec![xp; 24]
    }

    #[test]
    fn crossing_a_milestone_emits_one_event() {
        // Level 9 -> level 10 crosses the first milestone.
        let previous = skills_xp(xp_for_level(9) as i32);
        let current = skills_xp(xp_for_level(10) as i32);

        let events = diff_skills(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EVENT_TYPE_LEVEL_UP);
        assert_eq!(events[0].payload["skill"], json!("Agility"));
        assert_eq!(events[0].payload["level"], json!(10));
    }

    #[test]
    fn staying_within_a_milestone_band_emits_nothing() {
        let previous = skills_xp(xp_for_level(11) as i32);
        let current = skills_xp(xp_for_level(15) as i32);
        assert!(diff_skills(&previous, &current).is_empty());
    }

    #[test]
    fn jumping_multiple_thresholds_at_once_backfills_every_one() {
        let previous = skills_xp(xp_for_level(68) as i32);
        let current = skills_xp(xp_for_level(81) as i32);

        let events = diff_skills(&previous, &current);
        let levels: Vec<_> = events.iter().map(|event| event.payload["level"].clone()).collect();
        assert_eq!(levels, vec![json!(70), json!(80)]);
    }

    #[test]
    fn a_stale_baseline_spanning_every_skill_is_flagged_implausible() {
        // A stale/wrong `previous` (e.g. an unrelated account's old row) diffed against a
        // near-maxed character crosses every milestone on every skill at once - nothing like
        // real play produces this in a single heartbeat.
        let previous = all_skills_xp(xp_for_level(1) as i32);
        let current = all_skills_xp(xp_for_level(99) as i32);

        let events = diff_skills(&previous, &current);
        assert!(looks_like_stale_skills_baseline(&events));
    }

    #[test]
    fn a_genuine_multi_skill_jump_is_not_flagged_implausible() {
        // A raid completion or similar nudging a handful of skills over one threshold each is
        // real, plausible play and must not trip the stale-baseline guard.
        let previous = skills_xp(xp_for_level(68) as i32);
        let current = skills_xp(xp_for_level(81) as i32);

        let events = diff_skills(&previous, &current);
        assert!(!looks_like_stale_skills_baseline(&events));
    }

    #[test]
    fn reaching_99_emits_the_99_milestone() {
        let previous = skills_xp(xp_for_level(98) as i32);
        let current = skills_xp(xp_for_level(99) as i32);

        let events = diff_skills(&previous, &current);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["level"], json!(99));
    }

    #[test]
    fn a_skill_slot_missing_from_the_previous_snapshot_is_skipped() {
        let previous = vec![0i32; 5]; // Shorter than current - index 10 (Herblore) is unseen.
        let mut current = vec![0i32; 24];
        current[10] = xp_for_level(99) as i32;
        assert!(diff_skills(&previous, &current).is_empty());
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
            slayer_task: None,
            portrait_last_update: None,
            last_updated: None,
            events: None,
            interactions: None,
            object_interactions: None,
            alerts: None,
            notable_drops: None,
            pending: false,
        };

        assert!(diff_progress(&previous, &current).is_empty());
    }
}
