// Small curated static dataset, not server-scraped - see set_bonuses.json's sibling comment in
// the feature notes: set bonuses aren't in the wiki's per-item Infobox Bonuses template, so this
// mirrors item_data.json's loading convention (fetch a static /data/*.json file once, cache it)
// rather than adding server-side reference data for a modest, hand-curated list.
let setsPromise = null;

function loadSets() {
  if (!setsPromise) {
    setsPromise = fetch("/data/set_bonuses.json").then((response) => response.json());
  }
  return setsPromise;
}

/**
 * Given the item ids currently equipped, returns every curated set with its completion status:
 * `active` (every piece equipped), `partial` (some but not all pieces equipped, with
 * `missingItemIds` listing what's left), or neither (no pieces equipped at all).
 */
export async function detectActiveSets(equippedItemIds) {
  const sets = await loadSets();
  const equipped = new Set(equippedItemIds);

  return sets.map((set) => {
    const missingItemIds = set.itemIds.filter((itemId) => !equipped.has(itemId));
    return {
      name: set.name,
      effect: set.effect,
      itemIds: set.itemIds,
      missingItemIds,
      active: missingItemIds.length === 0,
      partial: missingItemIds.length > 0 && missingItemIds.length < set.itemIds.length,
    };
  });
}
