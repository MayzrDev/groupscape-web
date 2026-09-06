//! Combat-achievement boss -> task-id catalog.
//!
//! NOTE: `content/combat_achievement_tasks.json` is a manual-sync duplicate of the site's copy
//! under `site/public/data/` - the server can't fetch it at request time and the server Docker
//! build context doesn't include `site/`. Same tradeoff already accepted by
//! [`crate::collection_log_content`] / [`crate::quest_ids`] / [`crate::notable_npcs`].

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct RawTask {
    id: i64,
    name: String,
    #[serde(default)]
    boss: Option<String>,
}

// Tasks with no `boss` (general achievements) or `boss: "Other"` aren't tied to a specific
// monster, so there's nothing to detect "every task for this boss" completion against - mirrors
// `NON_BOSS_GROUPS` in `site/src/data/combat-achievement.js`.
const NON_BOSS_GROUP: &str = "Other";

/// Boss name -> the task ids that belong to it, across all tiers. Sorted by boss name so a
/// multi-boss upload persists deterministically.
static BOSS_GROUPS: LazyLock<Vec<(String, Vec<i64>)>> = LazyLock::new(|| {
    let catalog: HashMap<String, Vec<RawTask>> =
        serde_json::from_str(include_str!("content/combat_achievement_tasks.json"))
            .expect("content/combat_achievement_tasks.json must parse");

    let mut groups: HashMap<String, Vec<i64>> = HashMap::new();
    for tasks in catalog.values() {
        for task in tasks {
            let Some(boss) = task.boss.as_deref() else {
                continue;
            };
            if boss == NON_BOSS_GROUP {
                continue;
            }
            groups.entry(boss.to_string()).or_default().push(task.id);
        }
    }

    let mut groups: Vec<(String, Vec<i64>)> = groups.into_iter().collect();
    groups.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    groups
});

pub fn boss_groups() -> &'static [(String, Vec<i64>)] {
    &BOSS_GROUPS
}

/// Task id -> display name, for the per-task Discord webhook post.
static TASK_NAMES: LazyLock<HashMap<i64, String>> = LazyLock::new(|| {
    let catalog: HashMap<String, Vec<RawTask>> =
        serde_json::from_str(include_str!("content/combat_achievement_tasks.json"))
            .expect("content/combat_achievement_tasks.json must parse");

    catalog
        .into_values()
        .flatten()
        .map(|task| (task.id, task.name))
        .collect()
});

pub fn task_name(task_id: i64) -> Option<&'static str> {
    TASK_NAMES.get(&task_id).map(String::as_str)
}

/// Task id -> wiki-sourced completion percentage (`content/combat_achievement_completion.json`,
/// scraped from the "Comp%" column of https://oldschool.runescape.wiki/w/Combat_Achievements/All_tasks
/// - itself described there as "Estimated percentage of players (with at least one CA completion)
/// who have completed this achievement, based on wiki crowdsourcing", so this is an estimate, not
/// an official Jagex figure). Keyed by the wiki's `data-ca-task-id`, which matches the game's own
/// task id used throughout this codebase (verified against a handful of known tasks when this was
/// built). Not refreshed automatically - re-scrape and replace the JSON file if it goes stale.
static TASK_COMPLETION: LazyLock<HashMap<i64, f64>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("content/combat_achievement_completion.json"))
        .expect("content/combat_achievement_completion.json must parse")
});

pub fn completion_percent(task_id: i64) -> Option<f64> {
    TASK_COMPLETION.get(&task_id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boss_groups_are_loaded_with_tasks() {
        let groups = boss_groups();
        assert!(groups.len() > 50);
        assert!(groups.iter().all(|(_, tasks)| !tasks.is_empty()));
        assert!(groups.iter().any(|(boss, _)| boss == "Barrows"));
    }

    #[test]
    fn general_and_other_are_excluded() {
        let groups = boss_groups();
        assert!(groups.iter().all(|(boss, _)| boss != "General" && boss != "Other"));
    }

    #[test]
    fn completion_percent_is_loaded_for_known_task() {
        // Zulrah Adept - checked against https://oldschool.runescape.wiki/w/Combat_Achievements/All_tasks
        assert_eq!(completion_percent(224), Some(38.9));
    }

    #[test]
    fn completion_percent_covers_every_known_task() {
        for task_id in TASK_NAMES.keys() {
            assert!(completion_percent(*task_id).is_some(), "missing completion% for task {task_id}");
        }
    }
}
