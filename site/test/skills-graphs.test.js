import { describe, expect, it } from "vitest";
import { windowForPeriod, formatLeaderboardValue } from "../src/skills-graphs/skills-graphs";

describe("skills-graphs windowForPeriod", () => {
  it("maps each period to the leaderboard's 3 windows on the wire, with Month/Year collapsing to all_time", () => {
    expect(windowForPeriod.Hour1).toBe("daily");
    expect(windowForPeriod.Hour6).toBe("daily");
    expect(windowForPeriod.Hour12).toBe("daily");
    expect(windowForPeriod.Day).toBe("daily");
    expect(windowForPeriod.Week).toBe("weekly");
    expect(windowForPeriod.Month).toBe("all_time");
    expect(windowForPeriod.Year).toBe("all_time");
  });
});

describe("skills-graphs formatLeaderboardValue", () => {
  it("formats as a plain locale number", () => {
    expect(formatLeaderboardValue(1234567)).toBe("1,234,567");
  });
});
