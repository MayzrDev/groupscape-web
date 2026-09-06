use std::collections::HashSet;
use std::sync::LazyLock;

use crate::drop_rates::slugify_npc_name;

/// Curated set of chest/instance reward sources - RuneLite reports these as
/// `LootRecordType.EVENT` (source name, e.g. "Chambers of Xeric"), never correlated to any NPC
/// kill, so they need their own allowlist distinct from [`crate::notable_npcs`]'s kill-feed gate.
/// Kept in sync manually with the plugin's `ChestLootSourceNames.NAMES`.
static CHEST_SOURCES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut names: Vec<&str> = vec![
        "Chambers of Xeric",
        "Theatre of Blood",
        "Tombs of Amascut",
        "Barrows",
        "The Gauntlet",
        "The Corrupted Gauntlet",
        "Wintertodt",
        "Tempoross",
        "Zalcano",
        "Guardians of the Rift",
        "Fortis Colosseum",
        "Hunters' loot sack (basic)",
        "Hunters' loot sack (adept)",
        "Hunters' loot sack (expert)",
        "Hunters' loot sack (master)",
    ];
    names.drain(..).map(slugify_npc_name).collect()
});

/// Whether a plugin-reported source name matches this app's curated chest-loot allowlist.
pub fn is_known(source_name: &str) -> bool {
    CHEST_SOURCES.contains(&slugify_npc_name(source_name))
}

/// Slug -> display name for the loot-log page's source filter. Mirrors
/// [`crate::notable_npcs::names`]'s humanize-slug shape.
fn humanize_slug(slug: &str) -> String {
    slug.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn names() -> Vec<(String, String)> {
    let mut names: Vec<(String, String)> = CHEST_SOURCES
        .iter()
        .map(|slug| (slug.clone(), humanize_slug(slug)))
        .collect();
    names.sort_by(|a, b| a.1.cmp(&b.1));
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_curated_chest_sources() {
        assert!(is_known("Chambers of Xeric"));
        assert!(is_known("Theatre of Blood"));
        assert!(is_known("Tombs of Amascut"));
        assert!(is_known("Barrows"));
        assert!(is_known("The Gauntlet"));
        assert!(is_known("The Corrupted Gauntlet"));
        assert!(is_known("Guardians of the Rift"));
        assert!(is_known("Fortis Colosseum"));
        assert!(is_known("Hunters' loot sack (basic)"));
        assert!(is_known("Hunters' loot sack (master)"));
    }

    #[test]
    fn rejects_unknown_sources() {
        assert!(!is_known("Trouble Brewing"));
        assert!(!is_known("Random event"));
    }
}
