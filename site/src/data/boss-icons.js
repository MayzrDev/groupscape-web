// Slugs (matching npc-slug.js's slugifyNpcName) that have a downloaded RuneLite-hiscore-style
// icon at /icons/hiscore/bosses/<slug>.png - the same small square icon RuneLite's Hiscore
// plugin panel and the official OSRS hiscores page show for each boss. Those come from the
// live game's sprite cache with no downloadable file of their own (see HiscoreSkill.java in
// runelite/runelite), so these are sourced from wise-old-man's bundled hiscore icon set
// (MIT licensed, github.com/wise-old-man/wise-old-man, app/public/img/metrics) instead.
export const BOSS_ICON_SLUGS = new Set([
  "abyssal_sire",
  "alchemical_hydra",
  "amoxliatl",
  "araxxor",
  "artio",
  "barrows",
  "branda_the_fire_queen",
  "bryophyta",
  "callisto",
  "calvarion",
  "cerberus",
  "chambers_of_xeric",
  "chaos_elemental",
  "chaos_fanatic",
  "commander_zilyana",
  "corporeal_beast",
  "corrupted_hunllef",
  "crazy_archaeologist",
  "crystalline_hunllef",
  "dagannoth_prime",
  "dagannoth_rex",
  "dagannoth_supreme",
  "dawn",
  "deranged_archaeologist",
  "doom_of_mokhaiotl",
  "duke_sucellus",
  "dusk",
  "eldric_the_ice_king",
  "fortis_colosseum",
  "general_graardor",
  "giant_mole",
  "grotesque_guardians",
  "guardians_of_the_rift",
  "hespori",
  "kalphite_queen",
  "king_black_dragon",
  "kraken",
  "kreearra",
  "kril_tsutsaroth",
  "mad_angel",
  "maggot_king",
  "moons_of_peril",
  "nightmare_of_ashihama",
  "obor",
  "phantom_muspah",
  "phosanis_nightmare",
  "sarachnis",
  "scorpia",
  "scurrius",
  "shellbane_gryphon",
  "skotizo",
  "sol_heredit",
  "spindel",
  "tempoross",
  "the_chambers_of_xeric",
  "the_hueycoatl",
  "the_leviathan",
  "the_mimic",
  "the_nex",
  "the_royal_titans",
  "the_theatre_of_blood",
  "the_whisperer",
  "theatre_of_blood",
  "thermonuclear_smoke_devil",
  "tombs_of_amascut",
  "tzkal_zuk",
  "tztok_jad",
  "vardorvis",
  "venenatis",
  "vetion",
  "vorkath",
  "wintertodt",
  "yama",
  "zalcano",
  "zulrah",
]);

// Every clue tier has an icon - keyed directly by clue_tier (not slugified npc names).
export const CLUE_TIER_ICONS = new Set(["beginner", "easy", "medium", "hard", "elite", "master"]);

// Shared placeholder for any hiscores boss/clue/minigame entry with no matching local icon (see
// HISCORE_BOSS_ICON_GAPS below and activity-icons.js's own gap set) - a neutral "?" tile rather
// than blocking the boss-kc-panel tab on a missing asset.
export const HISCORE_ICON_PLACEHOLDER = "/icons/hiscore/activity-icon-placeholder.png";

// The live OSRS hiscores' `activities` boss names (server/src/hiscores.rs's ACTIVITY_DEFS `key`
// slugs - ground-truthed against a real hiscores response 2026-09-07) don't all slugify to an
// existing file in BOSS_ICON_SLUGS above - some are punctuation/spacing differences from the
// same boss, some are a raid's separate difficulty variant sharing its base raid's icon. Maps
// boss-kc-panel's hiscores `key` -> the existing icon slug to actually use.
export const HISCORE_BOSS_ICON_OVERRIDES = {
  barrows_chests: "barrows",
  chambers_of_xeric_cm: "chambers_of_xeric",
  lunar_chests: "moons_of_peril",
  mimic: "the_mimic",
  nex: "the_nex",
  nightmare: "nightmare_of_ashihama",
  the_gauntlet: "crystalline_hunllef",
  the_corrupted_gauntlet: "corrupted_hunllef",
  theatre_of_blood_hard_mode: "theatre_of_blood",
  tombs_of_amascut_expert_mode: "tombs_of_amascut",
  // "Colosseum Glory" (a minigame-category hiscores stat, not a boss) and "Rifts closed" reuse
  // existing boss-slot icons that already depict their minigame (Fortis Colosseum / Guardians of
  // the Rift) - see activity-icons.js's ACTIVITY_ICON_OVERRIDES for where these are consumed.
};

// hiscores `key`s ground-truthed as having no matching icon anywhere in this codebase's existing
// set (as of the 2026-09-07 ground-truthing pass) - rendered with HISCORE_ICON_PLACEHOLDER
// instead.
export const HISCORE_BOSS_ICON_GAPS = new Set([]);

// Resolves a boss-kc-panel hiscores `key` (boss category only) to its icon URL, applying the
// override table above and falling back to the shared placeholder for a documented gap.
export function hiscoreBossIconUrl(key) {
  if (HISCORE_BOSS_ICON_GAPS.has(key)) return HISCORE_ICON_PLACEHOLDER;
  const slug = HISCORE_BOSS_ICON_OVERRIDES[key] ?? key;
  return BOSS_ICON_SLUGS.has(slug) ? `/icons/hiscore/bosses/${slug}.png` : HISCORE_ICON_PLACEHOLDER;
}
