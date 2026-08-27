import { Quest } from "./quest";
import { Item } from "./item";
import { combatAchievement } from "./combat-achievement";

// The activity feed and the toast stack render the same six milestone event types, so the copy
// lives here once and each surface just supplies its own member/subject wrappers.
export const ACTIVITY_EVENT_TYPES = ["kill", "death", "quest", "diary", "combat_task", "collection_log"];

const BADGE_LABELS = {
  kill: "Kill",
  death: "Death",
  quest: "Quest",
  diary: "Diary",
  combat_task: "Combat task",
  collection_log: "Collection log",
};

// Secondary line under a toast title. Kills/deaths speak for themselves, so they have none.
const META_LABELS = {
  diary: "Achievement Diary",
  collection_log: "Collection log",
};

export function activityBadgeLabel(eventType) {
  return BADGE_LABELS[eventType] || eventType;
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

  switch (event.event_type) {
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
    case "quest":
      return `${member} completed ${wrapSubject(questNameFor(payload))}`;
    case "diary":
      return `${member} completed the ${wrapSubject(`${payload.region} ${payload.tier}`)} diary`;
    case "combat_task": {
      const { name, tierLabel } = combatTaskFor(payload);
      return `${member} completed ${wrapSubject(name)}${tierLabel ? ` (${tierLabel})` : ""}`;
    }
    case "collection_log":
      if (payload.kind === "page") {
        return `${member} completed the ${wrapSubject(payload.page)} collection log`;
      }
      return `${member} added ${wrapSubject(collectionLogItemNameFor(payload))} to their collection log`;
    default:
      return `${member} — ${event.event_type}`;
  }
}
