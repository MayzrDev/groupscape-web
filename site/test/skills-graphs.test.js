import { describe, expect, it } from "vitest";
import { formatLeaderboardValue } from "../src/skills-graphs/skills-graphs";

describe("skills-graphs formatLeaderboardValue", () => {
  it("formats as a plain locale number", () => {
    expect(formatLeaderboardValue(1234567)).toBe("1,234,567");
  });
});
