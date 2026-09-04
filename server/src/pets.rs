//! Detects a pet drop inside a [`crate::models::LootItem`] list, so `discord.rs` can pull it out
//! of the generic "Drops" line and post it as its own "Pet" embed instead.
//!
//! There's no `is_pet` flag anywhere upstream (the plugin reports pets the same as any other
//! loot item), so this matches on the item's display name against OSRS's fixed, slow-growing pet
//! roster. A miss here just means a new/unlisted pet posts through the ordinary Drops embed
//! instead of getting its own - not silent data loss, so the list doesn't need to be exhaustive
//! on day one.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::item_names;

static PET_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // Boss pets
        "Baby mole",
        "Callisto cub",
        "Hellpuppy",
        "Kalphite princess",
        "Olmlet",
        "Pet chaos elemental",
        "Pet dagannoth prime",
        "Pet dagannoth rex",
        "Pet dagannoth supreme",
        "Pet dark core",
        "Pet general graardor",
        "Pet k'ril tsutsaroth",
        "Pet kraken",
        "Pet kree'arra",
        "Pet penance queen",
        "Pet smoke devil",
        "Pet snakeling",
        "Pet zilyana",
        "Prince black dragon",
        "Scorpia's offspring",
        "Skotos",
        "Tzrek-jad",
        "Venenatis spiderling",
        "Vet'ion jr.",
        "Vorki",
        "Abyssal orphan",
        "Ikkle hydra",
        "Lil' zik",
        "Nexling",
        "Noon",
        "Phoenix",
        "Tumeken's guardian",
        "Whisperer",
        "Butch",
        "Baron",
        "Huberte",
        "Lil' creator",
        "Muphin",
        "Nid",
        "Smolcano",
        "Wisp",
        // Skilling pets
        "Beaver",
        "Herbi",
        "Rocky",
        "Giant squirrel",
        "Heron",
        "Rift guardian",
        "Rock golem",
        "Tangleroot",
        "Sraracha",
        "Baby chinchompa",
        "Chompy chick",
        "Abyssal protector",
        // Clue/minigame pets
        "Bloodhound",
        "Jal-nib-rek",
    ]
    .into_iter()
    .collect()
});

/// Whether an item id resolves to a known pet's display name.
pub fn is_pet_item(item_id: i32) -> bool {
    item_names::name(item_id).is_some_and(|name| PET_NAMES.contains(name))
}
