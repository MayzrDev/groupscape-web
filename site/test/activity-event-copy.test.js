import { afterEach, describe, expect, it } from "vitest";
// `data/item.js` sits in an import cycle with `api` -> `group-data` -> `member-data`, so the
// module graph has to be entered through `group-data` (the same way the app entrypoint does) for
// `Item` to be initialised before `member-data` reads it. Keep this import first.
import "../src/data/group-data";
import { Quest } from "../src/data/quest";
import { Item } from "../src/data/item";
import { combatAchievement } from "../src/data/combat-achievement";
import {
  activityBadgeLabel,
  activityEventDescription,
  activityMetaLabel,
} from "../src/data/activity-event-copy";

const event = (eventType, payload, memberName = "Bandos") => ({
  event_type: eventType,
  member_name: memberName,
  payload,
});

describe("activity event copy", () => {
  afterEach(() => {
    Quest.questData = undefined;
    combatAchievement.catalog = undefined;
    combatAchievement.taskLookup = undefined;
  });

  it("labels each milestone type", () => {
    expect(activityBadgeLabel(event("kill"))).toBe("Kill");
    expect(activityBadgeLabel(event("death"))).toBe("Death");
    expect(activityBadgeLabel(event("quest"))).toBe("Quest");
    expect(activityBadgeLabel(event("diary"))).toBe("Diary");
    expect(activityBadgeLabel(event("combat_task"))).toBe("Combat task");
    expect(activityBadgeLabel(event("collection_log"))).toBe("Collection log");
    expect(activityBadgeLabel(event("loot", { clueTier: "master" }))).toBe("Clue");
  });

  it("describes kills and deaths", () => {
    expect(activityEventDescription(event("kill", { npcName: "Vorkath", loot: [{ item_id: 1 }] }))).toBe(
      "Bandos killed Vorkath"
    );
    expect(activityEventDescription(event("death", { killerName: "Ice demon" }, "Woox"))).toBe(
      "Woox died to Ice demon"
    );
  });

  it("describes a quest completion by resolving the id against the loaded quest data", () => {
    Quest.questData = { 12: { name: "Dragon Slayer II" } };

    expect(activityEventDescription(event("quest", { quest_id: 12 }))).toBe("Bandos completed Dragon Slayer II");
  });

  it("describes a diary tier completion", () => {
    const diary = event("diary", { region: "Kandarin", tier: "Elite" }, "Torvesta");

    expect(activityEventDescription(diary)).toBe("Torvesta completed the Kandarin Elite diary");
    expect(activityMetaLabel(diary)).toBe("Achievement Diary");
  });

  it("describes a combat task completion with its tier", () => {
    combatAchievement.catalog = { grandmaster: [{ id: 300, name: "Zulrah's Big Sister" }] };
    const task = event("combat_task", { task_id: 300 }, "Framed");

    expect(activityEventDescription(task)).toBe("Framed completed Zulrah's Big Sister (Grandmaster)");
    expect(activityMetaLabel(task)).toBe("Grandmaster combat task");
  });

  it("describes completing every combat task for a boss", () => {
    const boss = event("combat_task", { kind: "boss", boss: "Zulrah" }, "Framed");

    expect(activityEventDescription(boss)).toBe("Framed completed all combat achievements for Zulrah");
    expect(activityMetaLabel(boss)).toBe("Combat achievements");
  });

  it("describes both collection log variants under one type", () => {
    Item.itemDetails = { 4151: { id: 4151, name: "Abyssal whip" } };
    const added = event("collection_log", { kind: "item", item_id: 4151, quantity: 1 });
    const completed = event("collection_log", { kind: "page", page: "Zulrah" });

    expect(activityEventDescription(added)).toBe("Bandos added Abyssal whip to their collection log");
    expect(activityEventDescription(completed)).toBe("Bandos completed the Zulrah collection log");
    expect(activityMetaLabel(added)).toBe("Collection log");
    expect(activityMetaLabel(completed)).toBe("Collection log");
  });

  it("describes a clue casket completion with its tier and gp value", () => {
    Item.itemDetails = { 4151: { id: 4151, name: "Abyssal whip" } };
    Item.gePrices = { 4151: 2000000 };
    const clue = event(
      "loot",
      { clueTier: "master", loot: [{ item_id: 4151, quantity: 1 }] },
      "Torvesta"
    );

    expect(activityEventDescription(clue)).toBe("Torvesta completed a Master clue — worth 2,000,000 gp");
  });

  it("wraps the member and subject with the caller's formatters", () => {
    Quest.questData = { 12: { name: "Dragon Slayer II" } };

    const html = activityEventDescription(event("quest", { quest_id: 12 }), {
      member: (name) => `<b>${name}</b>`,
      subject: (text) => `<i>${text}</i>`,
    });

    expect(html).toBe("<b>Bandos</b> completed <i>Dragon Slayer II</i>");
  });
});
