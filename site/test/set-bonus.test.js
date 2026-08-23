import { describe, expect, it, vi } from "vitest";

// set-bonus.js caches its fetched sets in module-level state, so each test needs a fresh
// module instance (via resetModules + dynamic import) to control what loadSets() resolves to.
let detectActiveSets;

const mockSets = async (sets) => {
  globalThis.fetch = vi.fn().mockResolvedValue({ json: vi.fn().mockResolvedValue(sets) });
  vi.resetModules();
  ({ detectActiveSets } = await import("../src/data/set-bonus"));
};

describe("set-bonus", () => {

  it("is active when every piece has a matching variant equipped", async () => {
    await mockSets([
      {
        name: "Blood Moon Armour Set",
        pieces: [[29028, 29047], [29022, 29043], [29025, 29045], [28997]],
        effect: "effect text",
      },
    ]);

    // Mix of New-state and Used-state ids across pieces, still a complete set.
    const result = await detectActiveSets([29028, 29043, 29045, 28997]);

    expect(result[0].active).toBe(true);
    expect(result[0].partial).toBe(false);
    expect(result[0].missingItemIds).toEqual([]);
  });

  it("matches a piece via its New-state variant id alone", async () => {
    await mockSets([
      {
        name: "Blood Moon Armour Set",
        pieces: [[29028, 29047], [29022, 29043], [29025, 29045], [28997]],
        effect: "effect text",
      },
    ]);

    const result = await detectActiveSets([29028, 29022, 29025, 28997]);

    expect(result[0].active).toBe(true);
  });

  it("does not treat a broken-state id as satisfying a piece", async () => {
    await mockSets([
      {
        name: "Blood Moon Armour Set",
        pieces: [[29028, 29047], [29022, 29043], [29025, 29045], [28997]],
        effect: "effect text",
      },
    ]);

    // 29067 is the broken chestplate id - not in the piece's variant list.
    const result = await detectActiveSets([29028, 29067, 29025, 28997]);

    expect(result[0].active).toBe(false);
    expect(result[0].partial).toBe(true);
    expect(result[0].missingItemIds).toEqual([29022]);
  });

  it("is partial when some but not all pieces are equipped, reporting piece counts", async () => {
    await mockSets([
      {
        name: "Void Knight Equipment",
        pieces: [[8842, 24182], [8839, 24177], [8840, 24179], [11665, 24185]],
        effect: "effect text",
      },
    ]);

    const result = await detectActiveSets([8842, 8839]);

    expect(result[0].active).toBe(false);
    expect(result[0].partial).toBe(true);
    expect(result[0].pieceCount).toBe(4);
    expect(result[0].missingItemIds).toEqual([8840, 11665]);
  });

  it("is neither active nor partial when no pieces are equipped", async () => {
    await mockSets([
      {
        name: "Ferocious Gloves",
        pieces: [[22981]],
        effect: "effect text",
      },
    ]);

    const result = await detectActiveSets([995]);

    expect(result[0].active).toBe(false);
    expect(result[0].partial).toBe(false);
    expect(result[0].missingItemIds).toEqual([22981]);
  });
});
