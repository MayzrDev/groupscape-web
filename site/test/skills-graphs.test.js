import { describe, expect, it } from "vitest";
import { windowForPeriod, formatLeaderboardValue } from "../src/skills-graphs/skills-graphs";

describe("skills-graphs windowForPeriod", () => {
  it("maps each period to the leaderboard's 3 windows on the wire, with Month/Year collapsing to all_time", () => {
    expect(windowForPeriod.Day).toBe("daily");
    expect(windowForPeriod.Week).toBe("weekly");
    expect(windowForPeriod.Month).toBe("all_time");
    expect(windowForPeriod.Year).toBe("all_time");
  });
});

describe("skills-graphs formatLeaderboardValue", () => {
  it("formats gp_earned as currency-style", () => {
    expect(formatLeaderboardValue("gp_earned", 1234567)).toBe("1,234,567 gp");
  });

  it("formats xp and boss_kc as plain locale numbers with no gp suffix", () => {
    expect(formatLeaderboardValue("xp", 1234567)).toBe("1,234,567");
    expect(formatLeaderboardValue("boss_kc", 42)).toBe("42");
  });
});
