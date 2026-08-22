// Keys match RuneLite's `net.runelite.api.Prayer` enum names, sent as-is by the plugin
// (`ActivePrayersState`) through to the frontend. The newer "Ruinous Powers" prayer book
// (RP_* enum values) isn't mapped yet - see PRAYER_ICON_BASE fallback below.
const PRAYER_ICON_BASE = "/icons/prayers";

const PRAYERS = {
  THICK_SKIN: { name: "Thick Skin", icon: "thick_skin" },
  BURST_OF_STRENGTH: { name: "Burst of Strength", icon: "burst_of_strength" },
  CLARITY_OF_THOUGHT: { name: "Clarity of Thought", icon: "clarity_of_thought" },
  SHARP_EYE: { name: "Sharp Eye", icon: "sharp_eye" },
  MYSTIC_WILL: { name: "Mystic Will", icon: "mystic_will" },
  ROCK_SKIN: { name: "Rock Skin", icon: "rock_skin" },
  SUPERHUMAN_STRENGTH: { name: "Superhuman Strength", icon: "superhuman_strength" },
  IMPROVED_REFLEXES: { name: "Improved Reflexes", icon: "improved_reflexes" },
  RAPID_RESTORE: { name: "Rapid Restore", icon: "rapid_restore" },
  RAPID_HEAL: { name: "Rapid Heal", icon: "rapid_heal" },
  PROTECT_ITEM: { name: "Protect Item", icon: "protect_item" },
  HAWK_EYE: { name: "Hawk Eye", icon: "hawk_eye" },
  MYSTIC_LORE: { name: "Mystic Lore", icon: "mystic_lore" },
  STEEL_SKIN: { name: "Steel Skin", icon: "steel_skin" },
  ULTIMATE_STRENGTH: { name: "Ultimate Strength", icon: "ultimate_strength" },
  INCREDIBLE_REFLEXES: { name: "Incredible Reflexes", icon: "incredible_reflexes" },
  PROTECT_FROM_MAGIC: { name: "Protect from Magic", icon: "protect_from_magic" },
  PROTECT_FROM_MISSILES: { name: "Protect from Missiles", icon: "protect_from_missiles" },
  PROTECT_FROM_MELEE: { name: "Protect from Melee", icon: "protect_from_melee" },
  EAGLE_EYE: { name: "Eagle Eye", icon: "eagle_eye" },
  MYSTIC_MIGHT: { name: "Mystic Might", icon: "mystic_might" },
  RETRIBUTION: { name: "Retribution", icon: "retribution" },
  REDEMPTION: { name: "Redemption", icon: "redemption" },
  SMITE: { name: "Smite", icon: "smite" },
  CHIVALRY: { name: "Chivalry", icon: "chivalry" },
  DEADEYE: { name: "Deadeye", icon: "deadeye" },
  MYSTIC_VIGOUR: { name: "Mystic Vigour", icon: "mystic_vigour" },
  PIETY: { name: "Piety", icon: "piety" },
  PRESERVE: { name: "Preserve", icon: "preserve" },
  RIGOUR: { name: "Rigour", icon: "rigour" },
  AUGURY: { name: "Augury", icon: "augury" },
};

// Overhead/protection prayers - prioritized into the visible slots when the row runs out of
// room (see [[project_active_prayer_icons_side_panel]] memory for the approved spec).
const PRIORITY_PRAYERS = new Set([
  "PROTECT_FROM_MAGIC",
  "PROTECT_FROM_MISSILES",
  "PROTECT_FROM_MELEE",
  "RETRIBUTION",
  "REDEMPTION",
  "SMITE",
]);

export class Prayer {
  static details(prayerKey) {
    return PRAYERS[prayerKey] ?? { name: prayerKey.replaceAll("_", " "), icon: null };
  }

  static displayName(prayerKey) {
    return Prayer.details(prayerKey).name;
  }

  static iconUrl(prayerKey) {
    const icon = Prayer.details(prayerKey).icon;
    return icon ? `${PRAYER_ICON_BASE}/${icon}.png` : null;
  }

  static sortByPriority(prayerKeys) {
    return [...prayerKeys].sort((a, b) => {
      const aPriority = PRIORITY_PRAYERS.has(a) ? 0 : 1;
      const bPriority = PRIORITY_PRAYERS.has(b) ? 0 : 1;
      return aPriority - bPriority;
    });
  }
}
