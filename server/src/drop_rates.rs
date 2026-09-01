use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DropRateEntry {
    pub name: String,
    pub rarity: String,
    #[serde(default)]
    pub is_unique: bool,
    /// Wiki-sourced fixed per-kill drop rate (e.g. "1/508"), only present where the mechanic
    /// is a simple fixed roll. Absent for rewards gated by points/team size/thresholds
    /// (raid uniques, Barrows, Wintertodt/Tempoross reward pools) - never guessed.
    #[serde(default)]
    pub rate: Option<String>,
}

/// Curated per-boss drop tables ported from `groupscape-old`'s `content/drop-rates.json`
/// (OSRS Wiki production drop-rate bands), keyed by a slugified boss name -> item id.
/// Bosses/items absent here simply have no rarity data - callers treat that as "unknown",
/// never a guess.
static DROP_RATES: LazyLock<HashMap<String, HashMap<i32, DropRateEntry>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("content/drop_rates.json"))
        .expect("content/drop_rates.json must parse")
});

/// Matches a plugin-reported NPC name (e.g. "Kree'arra", "K'ril Tsutsaroth") to the
/// drop-rates table's boss-slug keys: lowercase, apostrophes dropped, everything else
/// collapsed to single underscores.
pub fn slugify_npc_name(npc_name: &str) -> String {
    let mut slug = String::with_capacity(npc_name.len());
    let mut last_was_sep = false;
    for ch in npc_name.chars() {
        if ch == '\'' {
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            slug.push('_');
            last_was_sep = true;
        }
    }
    slug.trim_matches('_').to_string()
}

pub fn lookup(npc_name: &str, item_id: i32) -> Option<&'static DropRateEntry> {
    DROP_RATES.get(&slugify_npc_name(npc_name))?.get(&item_id)
}

/// Whether `npc_name` is one of the curated bosses in `content/drop_rates.json` - used to gate
/// the "Boss kills" Discord notification, which should skip regular NPCs.
pub fn is_boss(npc_name: &str) -> bool {
    DROP_RATES.contains_key(&slugify_npc_name(npc_name))
}

/// Ordinal rank for sorting loot summary rows by rarity, rarest first. Unknown/missing
/// rarities rank below every known tier rather than erroring.
pub fn rarity_rank(rarity: &str) -> u8 {
    match rarity {
        "very_rare" => 3,
        "rare" => 2,
        "uncommon" => 1,
        "common" => 0,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_apostrophes_and_spaces() {
        assert_eq!(slugify_npc_name("Kree'arra"), "kreearra");
        assert_eq!(slugify_npc_name("K'ril Tsutsaroth"), "kril_tsutsaroth");
        assert_eq!(slugify_npc_name("General Graardor"), "general_graardor");
        assert_eq!(slugify_npc_name("Vet'ion"), "vetion");
        assert_eq!(slugify_npc_name("Vorkath"), "vorkath");
    }

    #[test]
    fn looks_up_known_drop() {
        let entry = lookup("Vorkath", 11286).expect("draconic visage should be in vorkath's table");
        assert_eq!(entry.name, "Draconic visage");
        assert_eq!(entry.rarity, "very_rare");
        assert!(entry.is_unique);
    }

    #[test]
    fn unknown_npc_or_item_returns_none() {
        assert!(lookup("Some Random Npc", 1).is_none());
        assert!(lookup("Vorkath", 999_999).is_none());
    }

    #[test]
    fn rarity_rank_orders_rarest_highest() {
        assert!(rarity_rank("very_rare") > rarity_rank("rare"));
        assert!(rarity_rank("rare") > rarity_rank("uncommon"));
        assert!(rarity_rank("uncommon") > rarity_rank("common"));
        assert_eq!(rarity_rank("unknown"), rarity_rank("common"));
    }
}
