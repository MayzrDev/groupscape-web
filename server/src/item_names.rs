//! Item id -> display name / wiki link, for surfacing readable item names in Discord messages.
//!
//! `content/item_names.json` is a copy of `site/public/data/item_data.json` (same OSRS item
//! cache dump the frontend uses for icons/high-alch) - only the `name` field is read here. Keep
//! it in sync with the site copy when the item cache is refreshed; there's no shared build step
//! between the two crates yet.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct ItemEntry {
    name: String,
}

static ITEM_NAMES: LazyLock<HashMap<i32, String>> = LazyLock::new(|| {
    let raw: HashMap<String, ItemEntry> = serde_json::from_str(include_str!("content/item_names.json"))
        .expect("content/item_names.json must parse");
    raw.into_iter()
        .filter_map(|(id, entry)| id.parse::<i32>().ok().map(|id| (id, entry.name)))
        .collect()
});

pub fn name(item_id: i32) -> Option<&'static str> {
    ITEM_NAMES.get(&item_id).map(String::as_str)
}

/// Same URL scheme as the frontend's `Item.wikiLink` (`site/src/data/item.js`) - the wiki
/// resolves this lookup redirect to the item's actual page.
pub fn wiki_link(item_id: i32) -> String {
    format!("https://oldschool.runescape.wiki/w/Special:Lookup?type=item&id={item_id}")
}

/// Discord embed description markdown: the item's name as a link when known, otherwise the bare
/// id so a still-unrecognized item never renders as a dead link.
pub fn display(item_id: i32) -> String {
    match name(item_id) {
        Some(name) => format!("[{}]({})", name, wiki_link(item_id)),
        None => format!("item #{item_id}"),
    }
}
