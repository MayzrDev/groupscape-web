import { describe, expect, it } from "vitest";
import { Prayer } from "../src/data/prayer";

describe("prayer", () => {
  it("returns display name and icon url for known prayers", () => {
    expect(Prayer.displayName("PROTECT_FROM_MELEE")).toBe("Protect from Melee");
    expect(Prayer.iconUrl("PROTECT_FROM_MELEE")).toBe("/icons/prayers/protect_from_melee.png");
  });

  it("falls back to a readable name and null icon for unmapped prayers", () => {
    expect(Prayer.displayName("RP_DAMPEN_MAGIC")).toBe("RP DAMPEN MAGIC");
    expect(Prayer.iconUrl("RP_DAMPEN_MAGIC")).toBeNull();
  });

  it("sorts protection/overhead prayers before other active prayers", () => {
    const sorted = Prayer.sortByPriority(["RIGOUR", "PROTECT_FROM_MELEE", "PRESERVE", "SMITE"]);

    expect(sorted.slice(0, 2).sort()).toEqual(["PROTECT_FROM_MELEE", "SMITE"].sort());
    expect(sorted.slice(2).sort()).toEqual(["PRESERVE", "RIGOUR"].sort());
  });

  it("preserves input order when no prayers are active", () => {
    expect(Prayer.sortByPriority([])).toEqual([]);
  });
});
