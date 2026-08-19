import { beforeEach, describe, expect, it } from "vitest";
import { MemberData } from "../src/data/member-data";
import { Item } from "../src/data/item";
import { Quest } from "../src/data/quest";
import { pubsub } from "../src/data/pubsub";

describe("member-data", () => {
  let originalLookupByName;

  beforeEach(() => {
    originalLookupByName = Quest.lookupByName;
    Item.itemDetails = {
      4151: { id: 4151, name: "Abyssal whip" },
    };
  });

  it("publishes parsed collection log payload on collection_log_v2 updates", () => {
    const member = new MemberData("Alice");

    member.update({
      collection_log_v2: [{ id: 4151, quantity: 2 }],
    });

    const event = pubsub.getMostRecent("collection_log_v2:Alice");
    expect(event).toBeDefined();
    expect(event[0]).toBe(member.collectionLog);
    expect(member.collectionLog).toHaveLength(1);
    expect(member.collectionLog[0].id).toBe(4151);
    expect(member.collectionLog[0].quantity).toBe(2);
  });

  it("publishes parsed combat achievements payload on combat_achievements updates", () => {
    const member = new MemberData("Alice");

    member.update({
      combat_achievements: {
        tiers: { easy: true, medium: false, hard: false, elite: false, master: false, grandmaster: false },
        tasks: { "1234": true, "5678": false },
      },
    });

    const event = pubsub.getMostRecent("combatAchievements:Alice");
    expect(event).toBeDefined();
    expect(event[0]).toBe(member.combatAchievements);
    expect(member.combatAchievements.tiers.easy).toBe(true);
    expect(member.combatAchievements.tasks["1234"]).toBe(true);
  });

  it("publishes rich presence text on rich_presence updates", () => {
    const member = new MemberData("Alice");

    member.update({ rich_presence: "Talking to Reldo" });

    const event = pubsub.getMostRecent("richPresence:Alice");
    expect(event).toBeDefined();
    expect(event[0]).toBe("Talking to Reldo");
    expect(member.richPresence).toBe("Talking to Reldo");
  });

  it("publishes interacting updates when payload explicitly clears interacting", () => {
    const member = new MemberData("Alice");

    member.update({
      interacting: {
        name: "<col=ff0000>Goblin</col>",
        location: { x: 3200, y: 3200, plane: 0 },
        last_updated: new Date().toISOString(),
        ratio: 1,
        scale: 1,
      },
    });

    const updatedAttributes = member.update({
      interacting: null,
    });

    const event = pubsub.getMostRecent("interacting:Alice");
    expect(updatedAttributes.has("interacting")).toBe(true);
    expect(member.interacting).toBeNull();
    expect(event).toBeDefined();
    expect(event[0]).toBeNull();
  });

  it("returns false when quest lookup data is unavailable", () => {
    const member = new MemberData("Alice");
    member.quests = {};
    Quest.lookupByName = undefined;

    expect(member.hasQuestComplete("Cook's Assistant")).toBe(false);
  });

  it("returns false when member quest data is unavailable", () => {
    const member = new MemberData("Alice");
    Quest.lookupByName = new Map([["Cook's Assistant", "1"]]);

    expect(member.hasQuestComplete("Cook's Assistant")).toBe(false);
  });

  it("does not throw when combat level is computed with incomplete skills", () => {
    const member = new MemberData("Alice");
    member.skills = {
      Attack: { level: 99 },
      Strength: { level: 99 },
    };

    expect(() => member.computeCombatLevel()).not.toThrow();
    expect(member.combatLevel).toBeUndefined();
  });

  it("computes combat level when all required skills are present", () => {
    const member = new MemberData("Alice");
    member.skills = {
      Defence: { level: 99 },
      Hitpoints: { level: 99 },
      Prayer: { level: 99 },
      Attack: { level: 99 },
      Strength: { level: 99 },
      Ranged: { level: 99 },
      Magic: { level: 99 },
    };

    member.computeCombatLevel();

    expect(member.combatLevel).toBeGreaterThan(0);
  });

  it("parses packed potion storage data into its own source map", () => {
    Item.itemDetails[244] = { id: 244, name: "Attack potion(1)" };
    const member = new MemberData("Alice");

    const updated = member.update({ potion_storage: [{ id: 244, quantity: 6 }, { id: 244, quantity: 2 }] });

    expect(updated.has("potion_storage")).toBe(true);
    expect(member.itemQuantities.potionStorage.get(244)).toBe(8);
    expect(member.totalItemQuantity(244)).toBe(0);
  });

  it("publishes potion storage update on its own topic", () => {
    Item.itemDetails[244] = { id: 244, name: "Attack potion(1)" };
    const member = new MemberData("Alice");

    member.update({ potion_storage: [{ id: 244, quantity: 4 }] });

    const event = pubsub.getMostRecent("potionStorage:Alice");
    expect(event).toBeDefined();
    expect(event[0]).toHaveLength(1);
    expect(event[0][0].id).toBe(244);
    expect(event[0][0].quantity).toBe(4);
  });

  it("replaces prior potion storage with a new snapshot", () => {
    Item.itemDetails[244] = { id: 244, name: "Attack potion(1)" };
    Item.itemDetails[245] = { id: 245, name: "Strength potion(1)" };
    const member = new MemberData("Alice");

    member.update({ potion_storage: [{ id: 244, quantity: 6 }] });
    member.update({ potion_storage: [{ id: 245, quantity: 3 }] });

    expect(member.itemQuantities.potionStorage.get(244)).toBeUndefined();
    expect(member.itemQuantities.potionStorage.get(245)).toBe(3);
  });

  it("clears potion storage with an empty array without affecting normal items", () => {
    Item.itemDetails[244] = { id: 244, name: "Attack potion(1)" };
    Item.itemDetails[4151] = { id: 4151, name: "Abyssal whip" };
    const member = new MemberData("Alice");

    member.update({ inventory: [{ id: 4151, quantity: 2 }] });
    member.update({ potion_storage: [{ id: 244, quantity: 4 }] });

    expect(member.itemQuantities.potionStorage.get(244)).toBe(4);

    member.update({ potion_storage: [] });

    expect(member.itemQuantities.potionStorage.get(244)).toBeUndefined();
    expect(member.itemQuantities.potionStorage.size).toBe(0);
    expect(member.totalItemQuantity(4151)).toBe(2);
  });

  it("does not publish toasts from a member's first update", () => {
    const member = new MemberData("Alice");
    pubsub.unpublish("toast");

    member.update({
      skills: skillsPayload({ Attack: 100 }),
      quests: { 1: "FINISHED" },
      combat_achievements: { tiers: { easy: true }, tasks: {} },
    });

    expect(pubsub.getMostRecent("toast")).toBeUndefined();
  });

  it("publishes a level-up toast when a skill's level increases", () => {
    const member = new MemberData("Alice");
    member.update({ skills: skillsPayload({ Attack: 0 }) });
    pubsub.unpublish("toast");

    member.update({ skills: skillsPayload({ Attack: 100 }) });

    const [toast] = pubsub.getMostRecent("toast");
    expect(toast.type).toBe("level-up");
    expect(toast.memberName).toBe("Alice");
    expect(toast.skillName).toBe("Attack");
    expect(toast.level).toBeGreaterThan(1);
  });

  it("does not publish a level-up toast when a skill's level is unchanged", () => {
    const member = new MemberData("Alice");
    member.update({ skills: skillsPayload({ Attack: 50 }) });
    pubsub.unpublish("toast");

    member.update({ skills: skillsPayload({ Attack: 51 }) });

    expect(pubsub.getMostRecent("toast")).toBeUndefined();
  });

  it("publishes a quest toast when a quest transitions to FINISHED", () => {
    Quest.questData = { 1: { name: "Cook's Assistant" } };
    const member = new MemberData("Alice");
    member.update({ quests: { 1: "IN_PROGRESS" } });
    pubsub.unpublish("toast");

    member.update({ quests: { 1: "FINISHED" } });

    const [toast] = pubsub.getMostRecent("toast");
    expect(toast.type).toBe("quest");
    expect(toast.memberName).toBe("Alice");
    expect(toast.questName).toBe("Cook's Assistant");
  });

  it("does not publish a quest toast when an already-finished quest is resent", () => {
    Quest.questData = { 1: { name: "Cook's Assistant" } };
    const member = new MemberData("Alice");
    member.update({ quests: { 1: "FINISHED" } });
    pubsub.unpublish("toast");

    member.update({ quests: { 1: "FINISHED" } });

    expect(pubsub.getMostRecent("toast")).toBeUndefined();
  });

  it("publishes a combat-achievement toast when a tier newly completes", () => {
    const member = new MemberData("Alice");
    member.update({ combat_achievements: { tiers: { easy: false }, tasks: {} } });
    pubsub.unpublish("toast");

    member.update({ combat_achievements: { tiers: { easy: true }, tasks: {} } });

    const [toast] = pubsub.getMostRecent("toast");
    expect(toast.type).toBe("combat-achievement");
    expect(toast.memberName).toBe("Alice");
    expect(toast.tierKey).toBe("easy");
    expect(toast.tierLabel).toBe("Easy");
  });

  it("does not publish a combat-achievement toast when an already-complete tier is resent", () => {
    const member = new MemberData("Alice");
    member.update({ combat_achievements: { tiers: { easy: true }, tasks: {} } });
    pubsub.unpublish("toast");

    member.update({ combat_achievements: { tiers: { easy: true }, tasks: {} } });

    expect(pubsub.getMostRecent("toast")).toBeUndefined();
  });

  afterEach(() => {
    Quest.lookupByName = originalLookupByName;
  });
});

function skillsPayload(overrides) {
  const skillNames = [
    "Attack",
    "Defence",
    "Strength",
    "Hitpoints",
    "Ranged",
    "Prayer",
    "Magic",
    "Cooking",
    "Woodcutting",
    "Fletching",
    "Fishing",
    "Firemaking",
    "Crafting",
    "Smithing",
    "Mining",
    "Herblore",
    "Agility",
    "Thieving",
    "Slayer",
    "Farming",
    "Runecraft",
    "Hunter",
    "Construction",
    "Sailing",
    "Overall",
  ];
  const payload = {};
  for (const skillName of skillNames) {
    payload[skillName] = overrides[skillName] ?? 0;
  }
  return payload;
}
