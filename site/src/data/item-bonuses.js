import { api } from "./api";

// Module-level promise cache so every slot/render for the same item id shares one in-flight
// fetch (and, once resolved, the same cached result) instead of re-hitting the server - the
// server itself already de-dupes concurrent same-id scrapes across requesters, this just avoids
// redundant round-trips from this one client.
const cache = new Map();

/**
 * Fetches equipment bonuses for `itemId` (`{itemId, attack, defence, meleeStrength,
 * rangedStrength, magicDamage, prayer, attackSpeed}`), from the shared client cache when
 * already fetched. A failed fetch is not cached, so it can be retried on the next call.
 */
export function fetchItemBonuses(itemId) {
  if (!cache.has(itemId)) {
    const promise = api.getItemBonuses(itemId).catch((error) => {
      cache.delete(itemId);
      throw error;
    });
    cache.set(itemId, promise);
  }
  return cache.get(itemId);
}
