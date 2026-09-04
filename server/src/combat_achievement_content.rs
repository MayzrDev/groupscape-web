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
}
