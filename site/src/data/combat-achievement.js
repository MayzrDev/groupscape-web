export const COMBAT_ACHIEVEMENT_TIERS = [
  ["easy", "Easy"],
  ["medium", "Medium"],
  ["hard", "Hard"],
  ["elite", "Elite"],
  ["master", "Master"],
  ["grandmaster", "Grandmaster"],
];

class CombatAchievement {
  constructor() {}

  async initCatalog() {
    if (this.catalog) return;
    const response = await fetch("/data/combat_achievement_tasks.json");
    this.catalog = await response.json();
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
    return `https://oldschool.runescape.wiki/w/${encodeURIComponent(bossName.replace(/ /g, "_"))}`;
  }
}

export const combatAchievement = new CombatAchievement();
