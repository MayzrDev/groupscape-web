// Small curated static dataset, same rationale as set-bonus.js: undead (Salve amulet) and slayer
// (Black mask / Slayer helmet) bonuses aren't in the wiki's per-item Infobox Bonuses template this
// feature scrapes, so this is a hand-curated lookup rather than server-side reference data.
//
// Each item's percentage is its melee Attack/Strength bonus (the number the wiki and in-game item
// stats lead with). Imbued Salve/black mask variants also extend part of that bonus to ranged
// and/or magic, but never above the melee figure, so using one flat number per item is not a loss
// of information for a single-value display - it's already the largest of the per-style values.
let bonusesPromise = null;

function loadBonuses() {
  if (!bonusesPromise) {
    bonusesPromise = fetch("/data/target_bonuses.json").then((response) => response.json());
  }
  return bonusesPromise;
}

/**
 * Given the item ids currently equipped, returns `{undeadPercent, slayerPercent}` - the Salve
 * amulet-style undead bonus and the Black mask/Slayer helmet-style slayer bonus, each 0 when no
 * qualifying item is worn. Slayer task state isn't known client-side, so - matching how the
 * in-game equipment screen itself shows this number - the bonus is reported whenever a qualifying
 * item is worn, regardless of whether a task is active.
 */
export async function targetBonusesFor(equippedItemIds) {
  const bonuses = await loadBonuses();
  let undeadPercent = 0;
  let slayerPercent = 0;

  for (const itemId of equippedItemIds) {
    const undead = bonuses.undead[itemId];
    if (undead != null) undeadPercent = undead;
    const slayer = bonuses.slayer[itemId];
    if (slayer != null) slayerPercent = slayer;
  }

  return { undeadPercent, slayerPercent };
}
