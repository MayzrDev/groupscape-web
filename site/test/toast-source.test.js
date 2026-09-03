import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { toastSource } from "../src/data/toast-source";
import { api } from "../src/data/api";
import { pubsub } from "../src/data/pubsub";

describe("toast-source", () => {
  beforeEach(() => {
    toastSource.enabled = false;
    toastSource.interval = undefined;
    vi.spyOn(api, "getActivityEvents");
    pubsub.unpublish("toast");
  });

  afterEach(() => {
    toastSource.disable();
    vi.restoreAllMocks();
  });

  it("does not publish toasts for events already present on the first poll", async () => {
    api.getActivityEvents.mockResolvedValue([
      { id: 1, event_type: "kill", member_name: "Alice", payload: {} },
      { id: 2, event_type: "death", member_name: "Bob", payload: {} },
    ]);

    toastSource.enabled = true;
    await toastSource.poll();

    expect(pubsub.getMostRecent("toast")).toBeUndefined();
    expect(toastSource.lastEventId).toBe(2);
  });

  it("publishes a toast for each event newer than the baseline established on the first poll", async () => {
    api.getActivityEvents.mockResolvedValue([{ id: 1, event_type: "kill", member_name: "Alice", payload: {} }]);
    toastSource.enabled = true;
    await toastSource.poll();

    api.getActivityEvents.mockResolvedValue([
      { id: 1, event_type: "kill", member_name: "Alice", payload: {} },
      { id: 2, event_type: "death", member_name: "Bob", payload: {} },
    ]);
    const published = [];
    pubsub.subscribe("toast", (toast) => published.push(toast), false);

    await toastSource.poll();

    expect(published).toHaveLength(1);
    expect(published[0].type).toBe("death");
    expect(published[0].event.member_name).toBe("Bob");
    expect(toastSource.lastEventId).toBe(2);
  });

  it("routes combat_task events onto the existing combat-achievement toast type", async () => {
    api.getActivityEvents.mockResolvedValue([{ id: 1, event_type: "kill", member_name: "Alice", payload: {} }]);
    toastSource.enabled = true;
    await toastSource.poll();

    api.getActivityEvents.mockResolvedValue([
      { id: 1, event_type: "kill", member_name: "Alice", payload: {} },
      { id: 2, event_type: "combat_task", member_name: "Bob", payload: { task_id: 300 } },
      { id: 3, event_type: "diary", member_name: "Cara", payload: { region: "Kandarin", tier: "Elite" } },
      { id: 4, event_type: "collection_log", member_name: "Dee", payload: { kind: "page", page: "Zulrah" } },
    ]);
    const published = [];
    pubsub.subscribe("toast", (toast) => published.push(toast), false);

    await toastSource.poll();

    expect(published.map((toast) => toast.type)).toEqual(["combat-achievement", "diary", "collection_log"]);
    expect(published[0].event.event_type).toBe("combat_task");
  });

  it("routes clue casket loot events onto their own clue toast type", async () => {
    api.getActivityEvents.mockResolvedValue([{ id: 1, event_type: "kill", member_name: "Alice", payload: {} }]);
    toastSource.enabled = true;
    await toastSource.poll();

    api.getActivityEvents.mockResolvedValue([
      { id: 1, event_type: "kill", member_name: "Alice", payload: {} },
      { id: 2, event_type: "loot", member_name: "Bob", payload: { clueTier: "hard", loot: [] } },
    ]);
    const published = [];
    pubsub.subscribe("toast", (toast) => published.push(toast), false);

    await toastSource.poll();

    expect(published).toHaveLength(1);
    expect(published[0].type).toBe("clue");
    expect(published[0].event.event_type).toBe("loot");
  });

  it("does nothing once disabled", async () => {
    toastSource.enabled = false;
    await toastSource.poll();

    expect(api.getActivityEvents).not.toHaveBeenCalled();
  });
});
