export const COMBAT_ACHIEVEMENT_TIERS = [
  ["easy", "Easy"],
  ["medium", "Medium"],
  ["hard", "Hard"],
  ["elite", "Elite"],
  ["master", "Master"],
  ["grandmaster", "Grandmaster"],
];

const TIER_POINTS = {
  easy: 1,
  medium: 2,
  hard: 3,
  elite: 4,
  master: 5,
  grandmaster: 6,
};

// Total points required to unlock each tier's rewards, per the OSRS Combat Achievements reward system.
const TIER_UNLOCK_THRESHOLD = {
  easy: 41,
  medium: 161,
  hard: 419,
  elite: 1075,
  master: 1940,
  grandmaster: 2672,
};

const NON_BOSS_GROUPS = new Set(["General", "Other"]);

function wikiTitle(name) {
  return encodeURIComponent(name.trim().replace(/\s+/g, "_"));
}

class CombatAchievement {
  constructor() {}

  async initCatalog() {
    if (this.catalog) return;
    const response = await fetch("/data/combat_achievement_tasks.json");
    this.catalog = await response.json();
    this.taskLookup = undefined;
  }

  // The catalog is keyed by tier, but persisted `combat_task` activity events only carry a task
  // id, so flatten it once (~1500 tasks) and cache the id -> { name, tier, tierLabel } lookup.
  taskById(taskId) {
    if (!this.catalog) return null;
    if (!this.taskLookup) {
      this.taskLookup = new Map();
      for (const [tierKey] of COMBAT_ACHIEVEMENT_TIERS) {
        for (const task of this.tasksForTier(tierKey)) {
          this.taskLookup.set(task.id, { ...task, tier: tierKey, tierLabel: this.tierLabel(tierKey) });
        }
      }
    }
    return this.taskLookup.get(Number(taskId)) || null;
  }

  tasksForTier(tierKey) {
    return this.catalog[tierKey] || [];
  }

  totalTasksForTier(tierKey) {
    return this.tasksForTier(tierKey).length;
  }

  bossGroupsForTier(tierKey) {
    const groups = new Map();
    for (const task of this.tasksForTier(tierKey)) {
      const bossName = task.boss || "General";
      if (!groups.has(bossName)) groups.set(bossName, []);
      groups.get(bossName).push(task);
    }

    const groupList = Array.from(groups.entries()).map(([boss, tasks]) => ({ boss, tasks }));
    groupList.sort((a, b) => {
      if (a.boss === "General") return 1;
      if (b.boss === "General") return -1;
      return a.boss.localeCompare(b.boss);
    });

    return groupList;
  }

  isTaskComplete(member, taskId) {
    return !!member?.combatAchievements?.tasks?.[String(taskId)];
  }

  isTierComplete(member, tierKey) {
    return !!member?.combatAchievements?.tiers?.[tierKey];
  }

  completedTaskCountForTier(member, tierKey) {
    return this.tasksForTier(tierKey).filter((task) => this.isTaskComplete(member, task.id)).length;
  }

  tierCompletionPercent(member, tierKey) {
    const total = this.totalTasksForTier(tierKey);
    if (!total) return 0;
    return Math.round((this.completedTaskCountForTier(member, tierKey) / total) * 100);
  }

  completedTierCount(member) {
    return COMBAT_ACHIEVEMENT_TIERS.filter(([key]) => this.isTierComplete(member, key)).length;
  }

  firstIncompleteTier(member) {
    const incomplete = COMBAT_ACHIEVEMENT_TIERS.find(([key]) => !this.isTierComplete(member, key));
    return (incomplete || COMBAT_ACHIEVEMENT_TIERS[0])[0];
  }

  tierLabel(tierKey) {
    return COMBAT_ACHIEVEMENT_TIERS.find(([key]) => key === tierKey)?.[1] || tierKey;
  }

  bossWikiUrl(bossName) {
    return `https://oldschool.runescape.wiki/w/${wikiTitle(bossName)}`;
  }

  taskWikiUrl(taskName) {
    return `https://oldschool.runescape.wiki/w/${wikiTitle(taskName)}`;
  }

  pointsForTask(tierKey) {
    return TIER_POINTS[tierKey] || 0;
  }

  earnedPointsForTier(member, tierKey) {
    return this.completedTaskCountForTier(member, tierKey) * this.pointsForTask(tierKey);
  }

  totalPointsForTier(tierKey) {
    return this.totalTasksForTier(tierKey) * this.pointsForTask(tierKey);
  }

  totalPoints(member) {
    return COMBAT_ACHIEVEMENT_TIERS.reduce((sum, [key]) => sum + this.earnedPointsForTier(member, key), 0);
  }

  nextUnlock(member) {
    const points = this.totalPoints(member);
    const next = COMBAT_ACHIEVEMENT_TIERS.map(([key, label]) => ({
      key,
      label,
      threshold: TIER_UNLOCK_THRESHOLD[key],
    })).find((tier) => points < tier.threshold);

    if (!next) return null;
    return { label: next.label, pointsRemaining: next.threshold - points };
  }

  allBossGroups() {
    const groups = new Map();
    for (const [tierKey] of COMBAT_ACHIEVEMENT_TIERS) {
      for (const task of this.tasksForTier(tierKey)) {
        const bossName = task.boss || "General";
        if (NON_BOSS_GROUPS.has(bossName)) continue;
        if (!groups.has(bossName)) groups.set(bossName, []);
        groups.get(bossName).push({ ...task, tier: tierKey });
      }
    }

    return Array.from(groups.entries())
      .map(([boss, tasks]) => ({ boss, tasks }))
      .sort((a, b) => a.boss.localeCompare(b.boss));
  }

  completedTaskCountForBoss(member, tasks) {
    return tasks.filter((task) => this.isTaskComplete(member, task.id)).length;
  }
}

export const combatAchievement = new CombatAchievement();
