import { describe, expect, it, vi } from "vitest";
import { SkillName } from "../src/data/skill";
import { computeXpGains, rankEntries } from "../src/data/xp-gain";

describe("xp-gain rankEntries", () => {
  it("ranks distinct values in descending order", () => {
    const ranked = rankEntries([
      { member_name: "a", value: 10 },
      { member_name: "b", value: 30 },
      { member_name: "c", value: 20 },
    ]);
    expect(ranked.map((e) => e.member_name)).toEqual(["b", "c", "a"]);
    expect(ranked.map((e) => e.rank)).toEqual([1, 2, 3]);
  });

  it("shares rank across ties and skips the next rank", () => {
    const ranked = rankEntries([
      { member_name: "a", value: 10 },
      { member_name: "b", value: 10 },
      { member_name: "c", value: 5 },
    ]);
    expect(ranked.map((e) => e.rank)).toEqual([1, 1, 3]);
  });

  it("breaks ties alphabetically by member name", () => {
    const ranked = rankEntries([
      { member_name: "zed", value: 10 },
      { member_name: "amy", value: 10 },
    ]);
    expect(ranked.map((e) => e.member_name)).toEqual(["amy", "zed"]);
  });
});

describe("xp-gain computeXpGains", () => {
  it("computes xp gained over the exact selected period, distinguishing hour windows", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-01T12:00:00Z"));

    const groupData = {
      members: new Map([["alice", { skills: { [SkillName.Overall]: { xp: 1000 } } }]]),
    };

    const skillDataForGroup = [
      {
        name: "alice",
        skill_data: [
          { time: new Date("2026-11-01T01:00:00Z"), data: { [SkillName.Overall]: 100 } },
          { time: new Date("2026-11-01T04:00:00Z"), data: { [SkillName.Overall]: 250 } },
          { time: new Date("2026-11-01T07:00:00Z"), data: { [SkillName.Overall]: 400 } },
          { time: new Date("2026-11-01T10:00:00Z"), data: { [SkillName.Overall]: 700 } },
        ],
      },
    ];

    const hour6 = computeXpGains(skillDataForGroup, groupData, SkillName.Overall, "Hour6");
    const hour12 = computeXpGains(skillDataForGroup, groupData, SkillName.Overall, "Hour12");

    expect(hour6[0].value).toBe(600);
    expect(hour12[0].value).toBe(900);
    expect(hour6[0].value).not.toBe(hour12[0].value);
  });
});
