import { HISCORE_ICON_PLACEHOLDER } from "./boss-icons";

// Icons for the boss-kc-panel's "Clues & Minigames" section - the live OSRS hiscores' clue
// scroll / minigame / leagues activities, which (unlike bosses) had no existing local icon
// anywhere in this codebase.
//
// Sourced the same way this project's existing `/icons/hiscore/bosses` and `/icons/hiscore/clues`
// sets were sourced (see the reference_wiki_hiscore_boss_icons project memory / boss-icons.js's
// header comment): the RuneLite-hiscore-style small square icon for each of these has no
// downloadable file on the OSRS Wiki itself (checked - most of these minigame/leagues stats have
// no wiki `File:<Name> icon.png` at all, e.g. "Grid Points"/"Deadman Points" are UI-only stats
// with no wiki page to hotlink from), so these come from wise-old-man's bundled hiscore icon set
// (MIT licensed, github.com/wise-old-man/wise-old-man, app/public/img/metrics) instead, same
// source and same filename convention as the boss set.
//
// Six of the seven clue tiers already have icons at /icons/hiscore/clues/<tier>.png (from an
// earlier pass); only "Clue Scrolls (all)" is new here. "Colosseum Glory" and "Rifts closed"
// reuse existing boss-slot icons (Fortis Colosseum / Guardians of the Rift) that already depict
// their minigame, rather than downloading a second copy of the same picture.
export const ACTIVITY_ICON_SLUGS = new Set([
  "clue_scrolls_all",
  "bounty_hunter_hunter",
  "bounty_hunter_rogue",
  "last_man_standing",
  "pvp_arena",
  "soul_wars_zeal",
  "collections_logged",
  "league_points",
]);

// boss-kc-panel hiscores `key` -> filename stem under /icons/hiscore/activities/, for keys whose
// slug doesn't already match a file in ACTIVITY_ICON_SLUGS above.
export const ACTIVITY_ICON_OVERRIDES = {
  clue_all: "clue_scrolls_all",
  lms_rank: "last_man_standing",
  pvp_arena_rank: "pvp_arena",
};

// hiscores `key`s reusing an existing boss-slot icon instead of one under
// /icons/hiscore/activities/ - see the header comment above.
export const ACTIVITY_ICON_BOSS_SLOT_REUSE = {
  colosseum_glory: "fortis_colosseum",
  rifts_closed: "guardians_of_the_rift",
};

// hiscores `key`s ground-truthed as having no matching icon anywhere (legacy/retired hiscores
// categories - Deadman mode and the old pre-rework Bounty Hunter (Legacy) split, plus "Grid
// Points", a leagues stat with no icon of its own in wise-old-man's set either) - rendered with
// the shared placeholder.
export const ACTIVITY_ICON_GAPS = new Set([
  "grid_points",
  "deadman_points",
  "bounty_hunter_legacy_hunter",
  "bounty_hunter_legacy_rogue",
]);

// Resolves a boss-kc-panel hiscores `key` (clue/minigame category only) to its icon URL.
export function activityIconUrl(key) {
  if (key.startsWith("clue_") && key !== "clue_all") {
    // "clue_beginner" -> "beginner", matching CLUE_TIER_ICONS' existing keying.
    const tier = key.slice("clue_".length);
    return `/icons/hiscore/clues/${tier}.png`;
  }
  if (ACTIVITY_ICON_GAPS.has(key)) return HISCORE_ICON_PLACEHOLDER;
  if (ACTIVITY_ICON_BOSS_SLOT_REUSE[key]) {
    return `/icons/hiscore/bosses/${ACTIVITY_ICON_BOSS_SLOT_REUSE[key]}.png`;
  }
  const slug = ACTIVITY_ICON_OVERRIDES[key] ?? key;
  return ACTIVITY_ICON_SLUGS.has(slug) ? `/icons/hiscore/activities/${slug}.png` : HISCORE_ICON_PLACEHOLDER;
}
