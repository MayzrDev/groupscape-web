//! Collection-log page catalog + duplicate-item remapping.
//!
//! NOTE: `content/collection_log_info.json` and `content/collection_log_duplicates.json` are
//! manual-sync duplicates of the site's copies under `site/public/data/` - the server can't fetch
//! them at request time and the server Docker build context doesn't include `site/`. Same tradeoff
//! already accepted by [`crate::quest_ids`] / [`crate::notable_npcs`].

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct RawPageItem {
    id: i32,
}

#[derive(Deserialize)]
struct RawPage {
    name: String,
    items: Vec<RawPageItem>,
}

#[derive(Deserialize)]
struct RawTab {
    pages: Vec<RawPage>,
}

/// Dupe item id -> canonical item id, built the same way `collection-log.js`'s `initLogInfo`
/// builds its `reverseMapping` (first canonical id wins for a dupe listed more than once).
static DUPLICATE_MAPPING: LazyLock<HashMap<i32, i32>> = LazyLock::new(|| {
    let raw: HashMap<String, Vec<i32>> =
        serde_json::from_str(include_str!("content/collection_log_duplicates.json"))
            .expect("content/collection_log_duplicates.json must parse");
    let mut mapping = HashMap::new();
    for (canonical_id, dupe_ids) in raw {
        let Ok(canonical_id) = canonical_id.parse::<i32>() else {
            continue;
        };
        for dupe_id in dupe_ids {
            mapping.entry(dupe_id).or_insert(canonical_id);
        }
    }
    mapping
});

/// Page name -> the canonicalised item ids that page requires. Unlike the client (which only
/// remaps the *player's* items, leaving page ids raw), both sides are canonicalised here so a
/// page listing a dupe id - e.g. the duplicated mining outfit across the Volcanic Mine and
/// Motherlode Mine pages - can actually be completed instead of never matching.
static PAGES: LazyLock<Vec<(String, Vec<i32>)>> = LazyLock::new(|| {
    let tabs: Vec<RawTab> = serde_json::from_str(include_str!("content/collection_log_info.json"))
        .expect("content/collection_log_info.json must parse");
    tabs.into_iter()
        .flat_map(|tab| tab.pages)
        .map(|page| {
            let items = page
                .items
                .into_iter()
                .map(|item| canonical_item_id(item.id))
                .collect();
            (page.name, items)
        })
        .collect()
});

/// Collapses a duplicated collection-log item id onto the id the log actually counts, mirroring
/// `PlayerLog`'s remap in `site/src/data/collection-log.js`.
pub fn canonical_item_id(item_id: i32) -> i32 {
    DUPLICATE_MAPPING.get(&item_id).copied().unwrap_or(item_id)
}

pub fn pages() -> &'static [(String, Vec<i32>)] {
    &PAGES
}

/// The page a (canonicalised) collection-log item id belongs to, plus that page's full item list -
/// used to show "X/Y items" progress on the Discord collection-log item embed. `item_id` is
/// expected to already be canonicalised (see `canonical_item_id`); an item listed on more than one
/// page (rare) just returns the first match in `PAGES`' order.
pub fn page_containing(item_id: i32) -> Option<(&'static str, &'static [i32])> {
    PAGES
        .iter()
        .find(|(_, items)| items.contains(&item_id))
        .map(|(name, items)| (name.as_str(), items.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_loaded_with_items() {
        let pages = pages();
        assert!(pages.len() > 50);
        assert!(pages.iter().all(|(_, items)| !items.is_empty()));
        assert!(pages.iter().any(|(name, _)| name == "Abyssal Sire"));
    }

    #[test]
    fn duplicate_ids_collapse_onto_their_canonical_id() {
        // 4178 is listed as a duplicate of the Abyssal whip (4151) in the shared data file.
        assert_eq!(canonical_item_id(4178), 4151);
        // An id with no duplicate entry passes through untouched.
        assert_eq!(canonical_item_id(4151), 4151);
    }

    #[test]
    fn page_containing_finds_the_owning_page() {
        let (page, items) = page_containing(4151).expect("abyssal whip should be on a page");
        assert_eq!(page, "Abyssal Sire");
        assert!(items.contains(&4151));
    }

    #[test]
    fn page_containing_is_none_for_unknown_item() {
        assert!(page_containing(-1).is_none());
    }
}
