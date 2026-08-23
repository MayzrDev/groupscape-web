/**
 * "View DPS" link ticket: opens a member's current gear + real combat levels in the OSRS Wiki's
 * DPS calculator, the same way RuneLite's own equipment-tab button does - POST a loadout to its
 * shortlink API, then open `?id=<shortlink>` (it resolves to the clean `/osrs-dps/` URL once
 * loaded). CORS is open on this endpoint (`access-control-allow-origin: *`), so this is called
 * straight from the browser with no backend proxy - ported from groupscape-old's
 * `dpsCalcLink.ts`, adapted to this site's `EquipmentSlot` numbering and `Skill` objects.
 */
const DPS_CALC_UI_URL = "https://tools.runescape.wiki/osrs-dps/";
const DPS_CALC_SHORTLINK_URL = "https://tools.runescape.wiki/osrs-dps/shortlink";

/** `player-equipment.js`'s `EquipmentSlot` id -> the DPS calculator's named equipment slot. */
export const SLOT_TO_DPS_CALC_KEY = {
  0: "head",
  1: "cape",
  2: "neck",
  13: "ammo",
  3: "weapon",
  4: "body",
  5: "shield",
  7: "legs",
  9: "hands",
  10: "feet",
  12: "ring",
};

/** Maps a member's real skill levels (from `data/skill.js`'s `Skill.parseSkillData`) to the
 * calculator's short combat-skill keys. `null` if the member hasn't reported combat skills yet -
 * the calculator falls back to its own all-1 defaults in that case, same as a fresh loadout. */
export function combatSkillLevelsFromMember(member) {
  const skills = member?.skills;
  const required = ["Attack", "Defence", "Hitpoints", "Magic", "Mining", "Prayer", "Ranged", "Strength"];
  if (!skills || !required.every((name) => typeof skills[name]?.level === "number")) {
    return null;
  }

  return {
    atk: skills.Attack.level,
    def: skills.Defence.level,
    hp: skills.Hitpoints.level,
    magic: skills.Magic.level,
    mining: skills.Mining.level,
    prayer: skills.Prayer.level,
    ranged: skills.Ranged.level,
    str: skills.Strength.level,
  };
}

/** `equippedBySlot` is a `{ [EquipmentSlot]: Item }` map of currently-equipped, valid items. */
export function buildShortlinkPayload(equippedBySlot, skillLevels, rsn) {
  const equipment = {};
  for (const key of Object.values(SLOT_TO_DPS_CALC_KEY)) equipment[key] = null;
  for (const [slot, item] of Object.entries(equippedBySlot)) {
    const key = SLOT_TO_DPS_CALC_KEY[slot];
    if (key && item) equipment[key] = { id: item.id };
  }

  return {
    loadouts: [
      {
        equipment,
        // Calculator defaults everything to 1 for a still-unsynced skills snapshot - same
        // fallback its own UI uses for a fresh loadout.
        skills: skillLevels ?? { atk: 1, def: 1, hp: 10, magic: 1, mining: 1, prayer: 1, ranged: 1, str: 1 },
        buffs: { inWilderness: false, kandarinDiary: false, onSlayerTask: false, chargeSpell: false },
        name: rsn,
      },
    ],
  };
}

/** Opens a blank tab synchronously (on the click handler's stack) so the browser attributes it
 * to the user gesture, then redirects it once the shortlink resolves - the standard workaround
 * for popup blockers rejecting a post-`await` `window.open`. */
export function openDpsCalculator(equippedBySlot, skillLevels, rsn) {
  const tab = window.open("", "_blank");
  return fetch(DPS_CALC_SHORTLINK_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(buildShortlinkPayload(equippedBySlot, skillLevels, rsn)),
  })
    .then((res) => (res.ok ? res.json() : Promise.reject(new Error(`shortlink ${res.status}`))))
    .then((body) => {
      if (!tab || !body.data) return;
      tab.location.href = `${DPS_CALC_UI_URL}?id=${body.data}`;
    })
    .catch(() => {
      // Transient network failure - fall back to the plain calculator so the tab isn't left blank.
      if (tab) tab.location.href = DPS_CALC_UI_URL;
    });
}
