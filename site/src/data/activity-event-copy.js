import { Quest } from "./quest";
import { Item } from "./item";
import { combatAchievement } from "./combat-achievement";

const COLLECTION_LOG_WIKI_URL = "https://oldschool.runescape.wiki/w/Collection_log";

// The activity feed and the toast stack render the same six milestone event types, so the copy
// lives here once and each surface just supplies its own member/subject wrappers.
export const ACTIVITY_EVENT_TYPES = [
  "kill",
  "death",
  "quest",
  "diary",
  "combat_task",
  "collection_log",
  "clue",
  "raid",
];

const BADGE_LABELS = {
  kill: "Kill",
  death: "Death",
  quest: "Quest",
  diary: "Diary",
  combat_task: "Combat task",
  collection_log: "Collection log",
  clue: "Clue",
  // Raid completions display under their own per-raid icon (cox/tob/toa - see
  // `activityDisplayType`), but all three share the one "Raid" badge/filter chip.
  cox: "Raid",
  tob: "Raid",
  toa: "Raid",
};

// Secondary line under a toast title. Kills/deaths speak for themselves, so they have none.
const META_LABELS = {
  diary: "Achievement Diary",
  collection_log: "Collection log",
};

// Clue casket openings are stored as `loot` events (shared with chest openings - see LootEvent in
// models.rs) discriminated by `clueTier` being set, so every surface that wants to treat them as
// their own "clue" bucket has to derive that display type rather than reading `event_type` raw.
export function activityDisplayType(event) {
  if (event.event_type === "loot" && event.payload?.clueTier) return "clue";
  // Raid completions are stored under one shared `event_type: "raid"`, discriminated by
  // `payload.raidType` ("cox"/"tob"/"toa") so each raid can render its own icon/badge.
  if (event.event_type === "raid") return event.payload?.raidType || "raid";
  return event.event_type;
}

export function activityBadgeLabel(event) {
  const displayType = activityDisplayType(event);
  return BADGE_LABELS[displayType] || displayType;
}

export function clueTierLabel(tier) {
  return tier ? tier[0].toUpperCase() + tier.slice(1) : "";
}

export function clueValueFor(payload) {
  return (payload?.loot || []).reduce((total, entry) => total + new Item(entry.item_id).gePrice * entry.quantity, 0);
}

export function clueWikiUrl(tier) {
  return `https://oldschool.runescape.wiki/w/Clue_scroll_(${tier})`;
}

export function questNameFor(payload) {
  const questId = payload?.quest_id;
  return Quest.questData?.[questId]?.name || "a quest";
}

export function combatTaskFor(payload) {
  const task = combatAchievement.taskById(payload?.task_id);
  return {
    name: task?.name || "a combat task",
    tierLabel: task?.tierLabel || null,
  };
}

export function collectionLogItemNameFor(payload) {
  const itemId = payload?.item_id;
  return Item.itemDetails?.[itemId]?.name || "an item";
}

export function activityMetaLabel(event) {
  if (event.event_type === "combat_task") {
    const { tierLabel } = combatTaskFor(event.payload);
    return tierLabel ? `${tierLabel} combat task` : "Combat task";
  }
  return META_LABELS[event.event_type] || "";
}

// Where a toast/activity-feed row should link when clicked. Most event types don't have a
// dedicated per-item page, so they fall back to the activity feed itself.
const LINK_BY_EVENT_TYPE = {
  combat_task: "/group/combat-achievements",
  kill: "/group/loot-log",
  raid: "/group/loot-log",
};

export function activityLinkFor(event) {
  return LINK_BY_EVENT_TYPE[event.event_type] || "/group/activity";
}

export function npcWikiUrl(npcName) {
  return `https://oldschool.runescape.wiki/w/Special:Lookup?type=npc&name=${encodeURIComponent(npcName)}`;
}

// Category icons for the milestone event types - saved locally from the OSRS wiki's own
// icons for Quests, Achievement Diaries, Combat Achievements and the Collection log.
const ACTIVITY_ICONS = {
  quest: "/icons/activity/quest.png",
  diary: "/icons/activity/diary.png",
  combat_task: "/icons/activity/combat-task.png",
  collection_log: "/icons/activity/collection-log.png",
  clue: "/icons/activity/clue.png",
  cox: "/icons/activity/cox.png",
  tob: "/icons/activity/tob.png",
  // ToA has no single icon here - see `raidIconFor`, which picks one of the three below by
  // invocation level, matching the wiki's own per-mode iconography for this raid.
};

const RAID_NAMES = {
  cox: "Chambers of Xeric",
  tob: "Theatre of Blood",
  toa: "Tombs of Amascut",
};

// ToA's invocation-level bands, matching the wiki's own mode split: Entry 0-149, Normal 150-299,
// Expert 300-600. Unlike CoX/ToB (one fixed icon each), ToA's icon depends on which band the
// completed run's level falls into.
function raidIconFor(raidType, payload) {
  if (raidType !== "toa") return ACTIVITY_ICONS[raidType];
  const level = payload?.difficulty?.level || 0;
  if (level >= 300) return "/icons/activity/toa-expert.png";
  if (level >= 150) return "/icons/activity/toa-normal.png";
  return "/icons/activity/toa-entry.png";
}

function joinNames(names) {
  if (names.length <= 1) return names[0] || "";
  if (names.length === 2) return `${names[0]} and ${names[1]}`;
  return `${names.slice(0, -1).join(", ")}, and ${names[names.length - 1]}`;
}

// Sums every reporting member's own share of a merged raid completion's loot (see
// `RaidCompletionPayload` in models.rs) - mirrors `clueValueFor`'s "recompute from loot at
// render time" approach rather than trusting the server-cached `totalValue` blindly.
export function raidValueFor(payload) {
  return (payload?.participants || []).reduce(
    (total, participant) =>
      total + (participant.loot || []).reduce((sum, entry) => sum + new Item(entry.itemId).gePrice * entry.quantity, 0),
    0
  );
}

export function raidDifficultyLabel(payload) {
  const difficulty = payload?.difficulty;
  if (!difficulty) return null;
  if (difficulty.kind === "level") {
    return difficulty.level > 0 ? `level ${difficulty.level}` : "unknown level";
  }
  return difficulty.mode || null;
}

export function questWikiUrl(questName) {
  return `https://oldschool.runescape.wiki/w/${questName.replaceAll(" ", "_")}`;
}

// Matches the wiki's own diary page naming, e.g. https://oldschool.runescape.wiki/w/Ardougne_Diary#Hard
// (same pattern already used by diary-dialog.js for the per-tier section link).
export function diaryWikiUrl(region, tier) {
  return `https://oldschool.runescape.wiki/w/${region.replace(/ /g, "_")}_Diary#${tier}`;
}

const identity = (value) => value;

/// Phrasing convention for group-milestone copy across this app: "<member> <verb> <subject>"
/// (e.g. "Bandos killed Vorkath", "Torvesta completed the Kandarin Elite diary") - plain
/// past-tense sentences naming the actual boss/quest/diary/task/item, not a generic
/// "New activity" label. Discord webhook payloads (#35) and low-HP/wilderness alerts (#37-39)
/// should read the same way.
export function activityEventDescription(event, format = {}) {
  const wrapMember = format.member || identity;
  const wrapSubject = format.subject || identity;
  const member = wrapMember(event.member_name);
  const payload = event.payload || {};

  switch (activityDisplayType(event)) {
    case "kill": {
      // `npcName` is the server's camelCase `KillEvent` serialization; `npc_name` is accepted
      // too so either casing renders.
      const npc = payload.npcName || payload.npc_name;
      const noLoot = !payload.loot || payload.loot.length === 0;
      return `${member} killed ${wrapSubject(npc || "an NPC", "monster", npc && npcWikiUrl(npc))}${
        noLoot ? " — no loot" : ""
      }`;
    }
    case "death": {
      const killer = payload.killerName || payload.killer_name;
      return killer ? `${member} died to ${wrapSubject(killer, "death", npcWikiUrl(killer))}` : `${member} died`;
    }
    case "quest": {
      const questName = questNameFor(payload);
      return `${member} completed ${wrapSubject(questName, "quest", questWikiUrl(questName), ACTIVITY_ICONS.quest)}`;
    }
    case "diary":
      return `${member} completed the ${wrapSubject(
        `${payload.region} ${payload.tier}`,
        "diary",
        diaryWikiUrl(payload.region, payload.tier),
        ACTIVITY_ICONS.diary
      )} diary`;
    case "combat_task": {
      const { name, tierLabel } = combatTaskFor(payload);
      return `${member} completed ${wrapSubject(
        name,
        "combat_task",
        combatAchievement.taskWikiUrl(name),
        ACTIVITY_ICONS.combat_task
      )}${tierLabel ? ` (${tierLabel})` : ""}`;
    }
    case "collection_log":
      if (payload.kind === "page") {
        return `${member} completed the ${wrapSubject(
          payload.page,
          "collection_log",
          COLLECTION_LOG_WIKI_URL,
          ACTIVITY_ICONS.collection_log
        )} collection log`;
      }
      return `${member} added ${wrapSubject(
        collectionLogItemNameFor(payload),
        "collection_log",
        new Item(payload.item_id).wikiLink,
        Item.itemDetails?.[payload.item_id] ? Item.imageUrl(payload.item_id) : ACTIVITY_ICONS.collection_log
      )} to their collection log`;
    case "clue": {
      const tierLabel = clueTierLabel(payload.clueTier);
      const article = /^[aeiou]/i.test(tierLabel) ? "an" : "a";
      const value = clueValueFor(payload);
      return `${member} completed ${article} ${wrapSubject(
        `${tierLabel} clue`,
        "clue",
        clueWikiUrl(payload.clueTier),
        ACTIVITY_ICONS.clue
      )} — worth ${value.toLocaleString()} gp`;
    }
    case "cox":
    case "tob":
    case "toa": {
      const raidType = activityDisplayType(event);
      const raidName = RAID_NAMES[raidType];
      const participants = payload.participants || [];
      const names = participants.length ? participants.map((p) => p.memberName) : [event.member_name];
      // Every other branch above wraps a single RSN through `wrapMember` - here the joined list
      // stands in for "the member" since a merged raid completion has more than one, which is
      // why this doesn't just reuse the `member` binding computed at the top of this function.
      const nameList = wrapMember(joinNames(names));
      const total = raidValueFor(payload);
      const diffLabel = raidDifficultyLabel(payload);
      const suffix = diffLabel ? ` (${diffLabel})` : "";
      const gpSuffix = names.length > 1 ? "gp total" : "gp";
      return `${nameList} completed ${wrapSubject(
        raidName,
        raidType,
        npcWikiUrl(raidName),
        raidIconFor(raidType, payload)
      )}${suffix} — worth ${total.toLocaleString()} ${gpSuffix}`;
    }
    default:
      return `${member} — ${event.event_type}`;
  }
}
