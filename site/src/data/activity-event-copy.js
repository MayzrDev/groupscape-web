import { Quest } from "./quest";
import { Item } from "./item";
import { combatAchievement } from "./combat-achievement";

const COLLECTION_LOG_WIKI_URL = "https://oldschool.runescape.wiki/w/Collection_log";

// The activity feed and the toast stack render the same six milestone event types, so the copy
// lives here once and each surface just supplies its own member/subject wrappers.
export const ACTIVITY_EVENT_TYPES = ["kill", "death", "quest", "diary", "combat_task", "collection_log", "clue"];

const BADGE_LABELS = {
  kill: "Kill",
  death: "Death",
  quest: "Quest",
  diary: "Diary",
  combat_task: "Combat task",
  collection_log: "Collection log",
  clue: "Clue",
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
};

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
    default:
      return `${member} — ${event.event_type}`;
  }
}
