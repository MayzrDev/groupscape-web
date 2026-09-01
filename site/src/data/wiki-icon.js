// Resolves a boss's small square hiscore-style icon - the same art RuneLite's Hiscore plugin
// panel and the official OSRS hiscores page use for each boss (see HiscoreSkill.java in the
// runelite/runelite repo: those icons are sprite ids pulled from the live game cache at
// render time, not a downloadable file, so there's nothing to hotlink there directly). The wiki
// mirrors the same art under a predictable "<Boss Name> icon.png" file, hotlinked via its
// file-path redirect. Only the ~60 bosses tracked on the official hiscores have this file -
// anything else (regular monsters, chests, clues) 404s and the caller falls back to the plain
// source dot.
const WIKI_BASE = "https://oldschool.runescape.wiki/w/Special:FilePath";

// Keyed by boss name; caches the resolved (or null, on 404) URL so repeated mounts of the same
// boss's loot-log-group don't re-request the image.
const iconUrlCache = new Map();

export function wikiHiscoreIconUrl(bossName) {
  if (iconUrlCache.has(bossName)) return iconUrlCache.get(bossName);

  const url = `${WIKI_BASE}/${encodeURIComponent(`${bossName} icon.png`)}`;
  const promise = new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve(url);
    img.onerror = () => resolve(null);
    img.src = url;
  });

  iconUrlCache.set(bossName, promise);
  return promise;
}
