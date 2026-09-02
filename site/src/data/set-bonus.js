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
 * OSRS Wiki page titles are sentence case (first letter capitalized, everything else lowercase),
 * which most curated set names here conform to, so the page title (and therefore the URL) is
 * usually derivable. That derived title is a Grand Exchange trading-bundle page for a handful of
 * sets though (e.g. "Ahrim's armour set" is just the tradeable bundle item, not the equipment
 * page with the actual set effect) - those sets carry an explicit `wiki` override in
 * set_bonuses.json pointing at the real page instead.
 */
function wikiUrl(set) {
  if (set.wiki) return set.wiki;
  const title = set.name.charAt(0).toUpperCase() + set.name.slice(1).toLowerCase();
  return `https://oldschool.runescape.wiki/w/${encodeURIComponent(title.replace(/ /g, "_"))}`;
}

/**
 * Given the item ids currently equipped, returns every curated set with its completion status:
 * `active` (every piece equipped), `partial` (some but not all pieces equipped, with
 * `missingItemIds` listing what's left), or neither (no pieces equipped at all).
 *
 * Each set's `pieces` is an array of pieces, and each piece is itself an array of every item id
 * that counts as "wearing that piece" - degrade-state variants (e.g. Moon armour's New/Used ids),
 * locked-item variants, etc. A piece is satisfied if any one of its variant ids is equipped.
 */
export async function detectActiveSets(equippedItemIds) {
  const sets = await loadSets();
  const equipped = new Set(equippedItemIds);

  return sets.map((set) => {
    const missingPieces = set.pieces.filter((variantIds) => !variantIds.some((id) => equipped.has(id)));
    const missingItemIds = missingPieces.map((variantIds) => variantIds[0]);
    return {
      name: set.name,
      effect: set.effect,
      wikiUrl: wikiUrl(set),
      image: set.image,
      pieceCount: set.pieces.length,
      missingItemIds,
      active: missingPieces.length === 0,
      partial: missingPieces.length > 0 && missingPieces.length < set.pieces.length,
    };
  });
}
