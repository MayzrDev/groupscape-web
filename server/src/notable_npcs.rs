use std::collections::HashSet;
use std::sync::LazyLock;

use crate::drop_rates::slugify_npc_name;

/// Curated set of "notable" kills - bosses and major quest bosses - worth surfacing in the
/// activity feed and toast stream. Ordinary NPC kills are noisy (the plugin reports every kill,
/// not just boss ones) so the feed only shows a kill event when the NPC slugifies to one of
/// these; death events are never filtered by this list.
static NOTABLE_NPCS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut names: Vec<&str> = vec![
        // Bosses/activities that also have a curated drop table (content/drop_rates.json) -
        // kept in sync manually since the two lists serve different purposes (loot rarity vs.
        // feed noise). These are already slugs, but slugifying them again is a no-op.
        "vorkath",
        "zulrah",
        "cerberus",
        "general_graardor",
        "kril_tsutsaroth",
        "commander_zilyana",
        "kreearra",
        "alchemical_hydra",
        "king_black_dragon",
        "giant_mole",
        "kalphite_queen",
        "the_nex",
        "the_theatre_of_blood",
        "the_chambers_of_xeric",
        "wintertodt",
        "tempoross",
        "tombs_of_amascut",
        "kraken",
        "abyssal_sire",
        "thermonuclear_smoke_devil",
        "vetion",
        "callisto",
        "venenatis",
        "chaos_elemental",
        "scorpia",
        "barrows",
        "zalcano",
        "nightmare_of_ashihama",
        "phantom_muspah",
        // Other well-known bosses without a curated drop table.
        "Corporeal Beast",
        "TzTok-Jad",
        "TzKal-Zuk",
        "Sarachnis",
        "Skotizo",
        "Obor",
        "Bryophyta",
        "Dagannoth Rex",
        "Dagannoth Prime",
        "Dagannoth Supreme",
        "Grotesque Guardians",
        "Dusk",
        "Dawn",
        "Duke Sucellus",
        "The Leviathan",
        "The Whisperer",
        "Vardorvis",
        "Amoxliatl",
        "The Hueycoatl",
        "Yama",
        "Araxxor",
        // Major quest bosses.
        "Delrith",
        "Elvarg",
        "Galvek",
        "Dessous",
        "Skree",
        "Bouncer",
        "Justiciar Zachariah",
        "Sea Troll Queen",
        "Damis",
        "Fareed",
        "Ket'sal K'uk",
        "Kamil",
        "Char",
        "Flambeed",
        "Karamel",
        "Agrith Naar",
        "Sigmund",
        "Movario",
        "Slagilith",
        "Black Knight Titan",
        "Tanglefoot",
        "Nazastarool",
        "Treus Dayth",
        "The Inadequacy",
        "Glod",
        "Koschei the Deathless",
        "Insatiable Mutated Bloodveld",
    ];
    names.drain(..).map(slugify_npc_name).collect()
});

/// Whether a plugin-reported NPC name matches this app's curated notable-kill list (see
/// [`NOTABLE_NPCS`]). Used to hide non-boss kill spam from the activity feed/toasts while always
/// keeping death events.
pub fn is_notable(npc_name: &str) -> bool {
    NOTABLE_NPCS.contains(&slugify_npc_name(npc_name))
}

pub fn names() -> Vec<String> {
    let mut names: Vec<String> = NOTABLE_NPCS.iter().cloned().collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_curated_bosses_and_quest_bosses() {
        assert!(is_notable("Vorkath"));
        assert!(is_notable("Kree'arra"));
        assert!(is_notable("TzKal-Zuk"));
        assert!(is_notable("Delrith"));
        assert!(is_notable("Sea Troll Queen"));
    }

    #[test]
    fn rejects_ordinary_npcs() {
        assert!(!is_notable("Cow"));
        assert!(!is_notable("Man"));
        assert!(!is_notable("Goblin"));
    }
}
