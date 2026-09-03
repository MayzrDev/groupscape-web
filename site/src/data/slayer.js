import { bossIconFor } from "./activity-event-copy";

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

// Task monster -> local icon key, under /icons/slayer/monsters/<key>.png. Covers the common
// slayer task monster pool; anything not listed here falls back to UNKNOWN_TASK_ICON.
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
    const bossIcon = bossIconFor(taskName);
    if (bossIcon) return bossIcon;

    const key = SLAYER_MONSTER_ICONS[normalize(taskName)];
    return key ? `/icons/slayer/monsters/${key}.png` : UNKNOWN_TASK_ICON;
  }

  taskWikiUrl(taskName) {
    return `https://oldschool.runescape.wiki/w/${wikiTitle(taskName)}`;
  }
}

export const slayerData = new SlayerData();
