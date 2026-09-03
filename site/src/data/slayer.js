// Headshot/portrait icons for the current real OSRS slayer masters, downloaded from the OSRS
// wiki (Special:FilePath/<Master name>.png) and bundled locally under
// /icons/slayer/masters/<key>.png - see CLAUDE.md's asset-sourcing convention (no runtime
// wiki hotlinking).
const SLAYER_MASTER_ICONS = {
  turael: "turael",
  spria: "spria",
  mazchna: "mazchna",
  vannaka: "vannaka",
  chaeldar: "chaeldar",
  nieve: "nieve",
  steve: "steve",
  duradel: "duradel",
  "konar quo maten": "konar-quo-maten",
  krystilia: "krystilia",
};

// Task target (regular monster or assignable boss) -> local icon key, under
// /icons/slayer/monsters/<key>.png. Covers the full slayer task pool; anything not listed here
// falls back to UNKNOWN_TASK_ICON.
const SLAYER_MONSTER_ICONS = {
  "aberrant spectres": "aberrant-spectres",
  "abyssal demons": "abyssal-demons",
  ankou: "ankou",
  aviansies: "aviansies",
  banshees: "banshees",
  basilisks: "basilisks",
  bloodveld: "bloodveld",
  "blue dragons": "blue-dragons",
  "bronze dragons": "bronze-dragons",
  "cave bugs": "cave-bugs",
  "cave crawlers": "cave-crawlers",
  "cave horrors": "cave-horrors",
  "cave slimes": "cave-slimes",
  cockatrices: "cockatrices",
  "crawling hands": "crawling-hands",
  dagannoth: "dagannoth",
  "dark beasts": "dark-beasts",
  "dust devils": "dust-devils",
  "earth warriors": "earth-warriors",
  elves: "elves",
  "fever spiders": "fever-spiders",
  "fire giants": "fire-giants",
  gargoyles: "gargoyles",
  ghouls: "ghouls",
  "green dragons": "green-dragons",
  "harpie bug swarms": "harpie-bug-swarms",
  hellhounds: "hellhounds",
  "hill giants": "hill-giants",
  "infernal mages": "infernal-mages",
  "iron dragons": "iron-dragons",
  jellies: "jellies",
  "jungle horrors": "jungle-horrors",
  kalphites: "kalphites",
  kurasks: "kurasks",
  "lesser demons": "lesser-demons",
  lizardmen: "lizardmen",
  "mithril dragons": "mithril-dragons",
  mogres: "mogres",
  molanisks: "molanisks",
  "moss giants": "moss-giants",
  nechryael: "nechryael",
  ogres: "ogres",
  "otherworldly beings": "otherworldly-beings",
  pyrefiends: "pyrefiends",
  "red dragons": "red-dragons",
  rockslugs: "rockslugs",
  "sea snakes": "sea-snakes",
  shades: "shades",
  "skeletal wyverns": "skeletal-wyverns",
  "smoke devils": "smoke-devils",
  "spiritual creatures": "spiritual-creatures",
  "steel dragons": "steel-dragons",
  suqahs: "suqahs",
  "terror dogs": "terror-dogs",
  trolls: "trolls",
  turoth: "turoth",
  tzhaar: "tzhaar",
  vampyres: "vampyres",
  "wall beasts": "wall-beasts",
  "warped creatures": "warped-creatures",
  waterfiends: "waterfiends",
  wyrms: "wyrms",
  zombies: "zombies",

  // Every boss assignable via the generic "Boss" slayer task (the task unlocked by the 200-point
  // "Like a boss" reward, given by Duradel/Kuradal, Konar, Nieve/Steve, and Krystilia) - see
  // GroupScapeTrackerPlugin#resolveBossTaskId. Same render-icon style as every other task above,
  // downloaded from the wiki - not the smaller Hiscore-style icon used elsewhere on the site
  // (boss-icons.js's BOSS_ICON_SLUGS).
  "the leviathan": "the-leviathan",
  "the whisperer": "the-whisperer",
  vardorvis: "vardorvis",
  "duke sucellus": "duke-sucellus",
  "abyssal sire": "abyssal-sire",
  "alchemical hydra": "alchemical-hydra",
  cerberus: "cerberus",
  "thermonuclear smoke devil": "thermonuclear-smoke-devil",
  kraken: "kraken",
  "grotesque guardians": "grotesque-guardians",
  "dagannoth rex": "dagannoth-rex",
  "dagannoth prime": "dagannoth-prime",
  "dagannoth supreme": "dagannoth-supreme",
  "kalphite queen": "kalphite-queen",
  "giant mole": "giant-mole",
  sarachnis: "sarachnis",
  "k'ril tsutsaroth": "kril-tsutsaroth",
  "kree'arra": "kreearra",
  "commander zilyana": "commander-zilyana",
  "general graardor": "general-graardor",
  "vet'ion": "vetion",
  callisto: "callisto",
  venenatis: "venenatis",
  scorpia: "scorpia",
  "chaos elemental": "chaos-elemental",
  "chaos fanatic": "chaos-fanatic",
  "crazy archaeologist": "crazy-archaeologist",
  "king black dragon": "king-black-dragon",
  vorkath: "vorkath",
  zulrah: "zulrah",
  "phantom muspah": "phantom-muspah",
  araxxor: "araxxor",
};

const UNKNOWN_TASK_ICON = "/icons/slayer/monsters/unknown-task.png";

function normalize(name) {
  return `${name ?? ""}`.trim().toLowerCase();
}

function wikiTitle(name) {
  return encodeURIComponent(`${name}`.trim().replace(/\s+/g, "_"));
}

class SlayerData {
  masterIconUrl(masterName) {
    const key = SLAYER_MASTER_ICONS[normalize(masterName)];
    return key ? `/icons/slayer/masters/${key}.png` : null;
  }

  taskIconUrl(taskName) {
    const key = SLAYER_MONSTER_ICONS[normalize(taskName)];
    return key ? `/icons/slayer/monsters/${key}.png` : UNKNOWN_TASK_ICON;
  }

  taskWikiUrl(taskName) {
    return `https://oldschool.runescape.wiki/w/${wikiTitle(taskName)}`;
  }
}

export const slayerData = new SlayerData();
