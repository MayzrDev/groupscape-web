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
 * which every curated set name here already conforms to - so the page title, and therefore the
 * URL, is derivable rather than needing a hand-maintained field per set.
 */
function wikiUrl(name) {
  const title = name.charAt(0).toUpperCase() + name.slice(1).toLowerCase();
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
      wikiUrl: wikiUrl(set.name),
      pieceCount: set.pieces.length,
      missingItemIds,
      active: missingPieces.length === 0,
      partial: missingPieces.length > 0 && missingPieces.length < set.pieces.length,
    };
  });
}
