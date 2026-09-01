use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// RuneLite `QuestState` ordinal for a finished quest. The plugin uploads `quests` as one byte
/// per quest (the enum ordinal), and the site agrees on this slot too - `site/src/data/quest.js`'s
/// `QuestState` object has `FINISHED` as its third key, and `group-data.js` indexes
/// `Object.keys(QuestState)` by the raw byte.
pub const QUEST_STATE_FINISHED: u8 = 2;

#[derive(Deserialize)]
struct QuestEntry {
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    name: String,
}

/// Quest ids in the same order the plugin's `quests` byte array is indexed.
///
/// NOTE: `content/quest_data.json` is a manual-sync duplicate of
/// `site/public/data/quest_data.json` - the server can't fetch the site's copy at request time and
/// the server Docker build context doesn't include `site/`. Same tradeoff already accepted by
/// [`crate::notable_npcs`] vs `content/drop_rates.json`. The filter/sort below mirrors
/// `Quest.questIds` in `site/src/data/quest.js` verbatim.
static QUEST_IDS: LazyLock<Vec<i32>> = LazyLock::new(|| {
    let data: HashMap<String, QuestEntry> =
        serde_json::from_str(include_str!("content/quest_data.json"))
            .expect("content/quest_data.json must parse");
    let mut ids: Vec<i32> = data
        .iter()
        .filter(|(_, entry)| !entry.hidden)
        .filter_map(|(quest_id, _)| quest_id.parse::<i32>().ok())
        .collect();
    ids.sort_unstable();
    ids
});

static QUEST_NAMES: LazyLock<HashMap<i32, String>> = LazyLock::new(|| {
    let data: HashMap<String, QuestEntry> =
        serde_json::from_str(include_str!("content/quest_data.json"))
            .expect("content/quest_data.json must parse");
    data.into_iter()
        .filter_map(|(quest_id, entry)| quest_id.parse::<i32>().ok().map(|id| (id, entry.name)))
        .collect()
});

/// The quest id stored at `index` of an uploaded `quests` byte array, or `None` if the array is
/// longer than the catalog this server knows about (a newer plugin/game release).
pub fn quest_id_at(index: usize) -> Option<i32> {
    QUEST_IDS.get(index).copied()
}

/// Display name for a quest id - used by the Discord "Quest completions" notification, which
/// only has the id (see [`quest_id_at`]'s doc comment on why the frontend resolves this itself
/// for the activity feed instead).
pub fn quest_name(quest_id: i32) -> Option<&'static str> {
    QUEST_NAMES.get(&quest_id).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_ids_are_sorted_and_non_empty() {
        let first = quest_id_at(0).expect("catalog must not be empty");
        let second = quest_id_at(1).expect("catalog must have more than one quest");
        assert!(first < second);
        assert!(QUEST_IDS.len() > 100);
    }

    #[test]
    fn quest_id_out_of_range_is_none() {
        assert_eq!(quest_id_at(QUEST_IDS.len()), None);
    }
}
