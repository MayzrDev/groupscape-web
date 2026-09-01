/* global Chart */
import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { SkillName } from "../data/skill";
import { GroupData, groupData } from "../data/group-data";

// Day/Week/Month/Year is the single period toggle that drives both the chart and the
// leaderboard. The leaderboard only understands 3 windows on the wire, so this collapses
// Month and Year down onto the same "all_time" window (no new window value is introduced).
export const windowForPeriod = {
  Hour1: "daily",
  Hour6: "daily",
  Hour12: "daily",
  Day: "daily",
  Week: "weekly",
  Month: "all_time",
  Year: "all_time",
};

export function formatLeaderboardValue(metric, value) {
  if (metric === "gp_earned") {
    return `${Math.round(value).toLocaleString()} gp`;
  }
  return Math.round(value).toLocaleString();
}

// CoX's only split is Regular vs Challenge Mode; ToA buckets its raw invocation level into the
// game's own Entry/Normal/Expert tiers (see the server's `toa_tier`). ToB has no difficulty
// split yet (Hard Mode detection isn't implemented), so it gets no sub-select at all.
const raidDifficultyOptionsByType = {
  cox: [
    { value: "all", label: "All Difficulties" },
    { value: "regular", label: "Regular" },
    { value: "cm", label: "Challenge Mode" },
  ],
  toa: [
    { value: "all", label: "All Difficulties" },
    { value: "entry", label: "Entry" },
    { value: "normal", label: "Normal" },
    { value: "expert", label: "Expert" },
  ],
};

export class SkillsGraphs extends BaseElement {
  constructor() {
    super();
  }

  /* eslint-disable no-unused-vars */
  html() {
    const skillNames = Object.values(SkillName).sort((a, b) => {
      if (a === "Overall") return -1;
      if (b === "Overall") return 1;
      return a.localeCompare(b);
    });
    return `{{skills-graphs.html}}`;
  }
  /* eslint-enable no-unused-vars */

  connectedCallback() {
    super.connectedCallback();
    this.render();

    // Single source of truth for both the chart and the leaderboard.
    this.state = {
      period: "Day",
      metric: "xp",
      skill: "Overall",
      boss: "",
      raidType: "all",
      raidDifficulty: "all",
      groupBy: "member",
    };
    this.fetchGeneration = 0;

    this.chartContainer = this.querySelector(".skills-graphs__chart-container");
    this.periodButtons = this.querySelectorAll(".skills-graphs__period-btn");
    this.refreshButton = this.querySelector(".skills-graphs__refresh");
    this.metricSelect = this.querySelector(".skills-graphs__metric-select");
    this.skillSelect = this.querySelector(".skills-graphs__skill-select");
    this.skillSelectContainer = this.querySelector(".skills-graphs__sub-select-container--skill");
    this.bossSelect = this.querySelector(".skills-graphs__boss-select");
    this.bossSelectContainer = this.querySelector(".skills-graphs__sub-select-container--boss");
    this.raidTypeSelect = this.querySelector(".skills-graphs__raid-type-select");
    this.raidTypeSelectContainer = this.querySelector(".skills-graphs__sub-select-container--raid-type");
    this.raidDifficultySelect = this.querySelector(".skills-graphs__raid-difficulty-select");
    this.raidDifficultySelectContainer = this.querySelector(".skills-graphs__sub-select-container--raid-difficulty");
    this.groupByContainer = this.querySelector(".skills-graphs__sub-select-container--group-by");
    this.groupByButtons = this.querySelectorAll(".skills-graphs__group-by-btn");
    this.leaderboardList = this.querySelector(".skills-graphs__leaderboard-list");
    this.leaderboardEmpty = this.querySelector(".skills-graphs__leaderboard-empty");

    this.state.skill = this.skillSelect.value;
    this.state.raidType = this.raidTypeSelect.value;

    this.periodButtons.forEach((btn) => {
      this.eventListener(btn, "click", this.handlePeriodChange.bind(this));
    });
    this.eventListener(this.refreshButton, "click", this.handleRefreshClicked.bind(this));
    this.eventListener(this.metricSelect, "change", this.handleMetricChange.bind(this));
    this.eventListener(this.skillSelect, "change", this.handleSkillSelectChange.bind(this));
    this.eventListener(this.bossSelect, "change", this.handleBossSelectChange.bind(this));
    this.eventListener(this.raidTypeSelect, "change", this.handleRaidTypeChange.bind(this));
    this.eventListener(this.raidDifficultySelect, "change", this.handleRaidDifficultyChange.bind(this));
    this.groupByButtons.forEach((btn) => {
      this.eventListener(btn, "click", this.handleGroupByChange.bind(this));
    });

    this.updateRaidDifficultyOptions();
    this.updateSubSelectVisibility();
    this.triggerRefresh();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  updateSubSelectVisibility() {
    const isRaids = this.state.metric === "raid_completions";
    this.skillSelectContainer.classList.toggle("visible", this.state.metric === "xp");
    this.bossSelectContainer.classList.toggle("visible", this.state.metric === "boss_kc");
    this.raidTypeSelectContainer.classList.toggle("visible", isRaids);
    this.groupByContainer.classList.toggle("visible", isRaids);
    const hasDifficulty = isRaids && !!raidDifficultyOptionsByType[this.state.raidType];
    this.raidDifficultySelectContainer.classList.toggle("visible", hasDifficulty);
  }

  updateRaidDifficultyOptions() {
    const options = raidDifficultyOptionsByType[this.state.raidType];
    if (!options) {
      this.state.raidDifficulty = "all";
      return;
    }
    const previousValue = this.state.raidDifficulty;
    this.raidDifficultySelect.innerHTML = options.map((o) => `<option value="${o.value}">${o.label}</option>`).join("");
    const stillValid = options.some((o) => o.value === previousValue);
    this.raidDifficultySelect.value = stillValid ? previousValue : "all";
    this.state.raidDifficulty = this.raidDifficultySelect.value;
  }

  handleMetricChange() {
    this.state.metric = this.metricSelect.value;
    if (this.state.metric === "xp") {
      this.state.skill = this.skillSelect.value || "Overall";
    } else if (this.state.metric === "boss_kc") {
      this.state.boss = this.bossSelect.value || "";
    } else {
      this.state.boss = "";
    }
    if (this.state.metric === "raid_completions") {
      this.state.raidType = this.raidTypeSelect.value || "all";
      this.updateRaidDifficultyOptions();
    }
    this.updateSubSelectVisibility();
    this.triggerRefresh();
  }

  handleSkillSelectChange() {
    this.state.skill = this.skillSelect.value;
    this.triggerRefresh();
  }

  handleBossSelectChange() {
    this.state.boss = this.bossSelect.value;
    this.triggerRefresh();
  }

  handleRaidTypeChange() {
    this.state.raidType = this.raidTypeSelect.value;
    this.state.raidDifficulty = "all";
    this.updateRaidDifficultyOptions();
    this.updateSubSelectVisibility();
    this.triggerRefresh();
  }

  handleRaidDifficultyChange() {
    this.state.raidDifficulty = this.raidDifficultySelect.value;
    this.triggerRefresh();
  }

  handleGroupByChange(event) {
    this.state.groupBy = event.currentTarget.dataset.groupBy;
    this.groupByButtons.forEach((btn) => {
      btn.classList.toggle("active", btn.dataset.groupBy === this.state.groupBy);
    });
    this.triggerRefresh();
  }

  handlePeriodChange(event) {
    this.state.period = event.currentTarget.dataset.period;
    this.periodButtons.forEach((btn) => {
      btn.classList.toggle("active", btn.dataset.period === this.state.period);
    });
    this.triggerRefresh();
  }

  handleRefreshClicked() {
    this.triggerRefresh();
  }

  // Single pipeline entry point: bumps one generation counter shared by the chart fetch and
  // the leaderboard fetch so stale in-flight responses from a previous state are discarded
  // the same way the old two independent generation counters did.
  triggerRefresh() {
    const generation = ++this.fetchGeneration;
    this.fetchLeaderboard(generation);
    this.subscribeOnce("get-group-data", () => this.createChart(generation));
  }

  async fetchLeaderboard(generation) {
    try {
      const isXp = this.state.metric === "xp";
      const isRaids = this.state.metric === "raid_completions";
      const skillParam = isXp && this.state.skill && this.state.skill !== "Overall" ? this.state.skill : undefined;
      const bossParam = this.state.metric === "boss_kc" ? this.state.boss || undefined : undefined;
      const raidTypeParam = isRaids ? this.state.raidType : undefined;
      const raidDifficultyParam = isRaids ? this.state.raidDifficulty : undefined;

      const result = await api.getLeaderboard(
        this.state.metric,
        windowForPeriod[this.state.period] || "daily",
        bossParam,
        skillParam,
        raidTypeParam,
        raidDifficultyParam
      );
      if (generation !== this.fetchGeneration) return;
      this.renderBossOptions(result.available_bosses || []);
      this.renderLeaderboard(result.entries || []);
    } catch (err) {
      if (generation !== this.fetchGeneration) return;
      console.error(err);
      this.leaderboardList.innerHTML = "";
      this.leaderboardEmpty.textContent = `Failed to load ${err}`;
      this.leaderboardEmpty.classList.add("skills-graphs__leaderboard-empty--visible");
    }
  }

  renderBossOptions(bosses) {
    if (this.state.metric !== "boss_kc") return;
    const previousValue = this.bossSelect.value || this.state.boss;
    this.bossSelect.innerHTML = `
      <option value="">All Bosses</option>
      ${bosses.map((boss) => `<option value="${boss}">${boss}</option>`).join("")}
    `;
    this.bossSelect.value = previousValue;
  }

  renderLeaderboard(entries) {
    entries = entries.filter((entry) => entry.member_name !== "@SHARED");
    this.leaderboardList.innerHTML = "";
    this.leaderboardEmpty.textContent = "No data for this window yet.";
    this.leaderboardEmpty.classList.toggle("skills-graphs__leaderboard-empty--visible", entries.length === 0);

    for (const entry of entries) {
      const row = document.createElement("div");
      row.classList.add("skills-graphs__leaderboard-row");
      if (entry.rank === 1) row.classList.add("skills-graphs__leaderboard-row--top1");
      else if (entry.rank === 2) row.classList.add("skills-graphs__leaderboard-row--top2");
      else if (entry.rank === 3) row.classList.add("skills-graphs__leaderboard-row--top3");

      const rank = document.createElement("span");
      rank.classList.add("skills-graphs__leaderboard-rank");
      rank.textContent = entry.rank;
      row.appendChild(rank);

      if (groupData.members.has(entry.member_name)) {
        const icon = document.createElement("player-icon");
        icon.setAttribute("player-name", entry.member_name);
        row.appendChild(icon);
      }

      const name = document.createElement("span");
      name.classList.add("skills-graphs__leaderboard-name");
      name.textContent = entry.member_name;
      row.appendChild(name);

      const value = document.createElement("span");
      value.classList.add("skills-graphs__leaderboard-value");
      value.textContent = formatLeaderboardValue(this.state.metric, entry.value);
      row.appendChild(value);

      this.leaderboardList.appendChild(row);
    }
  }

  async createChart(generation) {
    if (generation !== this.fetchGeneration) return;
    this.querySelector(".skills-graphs__loader-overlay")?.remove();
    const overlay = document.createElement("div");
    overlay.classList.add("skills-graphs__loader-overlay");
    const loader = document.createElement("div");
    loader.classList.add("loader");
    loader.innerHTML = "<div></div><div></div><div></div><div></div>";
    overlay.appendChild(loader);
    this.appendChild(overlay);

    try {
      const isXp = this.state.metric === "xp";
      const isRaids = this.state.metric === "raid_completions";
      const dataPromise = isXp
        ? api.getSkillData(this.state.period)
        : api.getMetricData(
            this.state.metric,
            this.state.period,
            this.state.boss || undefined,
            isRaids ? this.state.raidType : undefined,
            isRaids ? this.state.raidDifficulty : undefined,
            isRaids ? this.state.groupBy : undefined
          );
      const [rawData] = await Promise.all([dataPromise, this.waitForChartjs()]);
      if (generation !== this.fetchGeneration) return;

      rawData.sort((a, b) => a.name.localeCompare(b.name));

      const skillGraph = document.createElement("skill-graph");
      skillGraph.setAttribute("data-period", this.state.period);
      skillGraph.setAttribute("metric", this.state.metric);

      if (isXp) {
        rawData.forEach((playerSkillData) => {
          playerSkillData.skill_data.forEach((x) => {
            x.time = new Date(x.time);
            x.data = GroupData.transformSkillsFromStorage(x.data);
          });
          playerSkillData.skill_data.sort((a, b) => b.time - a.time);
        });
        skillGraph.skillDataForGroup = rawData;
        skillGraph.setAttribute("skill-name", this.state.skill);
      } else {
        rawData.forEach((playerMetricData) => {
          playerMetricData.metric_data.forEach((x) => {
            x.time = new Date(x.time);
          });
          playerMetricData.metric_data.sort((a, b) => b.time - a.time);
        });
        skillGraph.metricDataForGroup = rawData;
        if (this.state.metric === "boss_kc") {
          skillGraph.setAttribute("boss", this.state.boss || "");
        }
        if (this.state.metric === "raid_completions") {
          skillGraph.setAttribute("raid-type", this.state.raidType || "all");
          skillGraph.setAttribute("raid-difficulty", this.state.raidDifficulty || "all");
          skillGraph.setAttribute("group-by", this.state.groupBy || "member");
        }
      }

      overlay.remove();
      this.chartContainer.innerHTML = "";
      Chart.defaults.scale.grid.borderColor = "rgba(255, 255, 255, 0)";
      const style = getComputedStyle(document.body);
      Chart.defaults.color = style.getPropertyValue("--primary-text");
      Chart.defaults.scale.grid.color = style.getPropertyValue("--graph-grid-border");

      this.chartContainer.appendChild(skillGraph);
    } catch (err) {
      overlay.remove();
      console.error(err);
      this.chartContainer.innerHTML = `Failed to load ${err}`;
    }
  }

  async waitForChartjs() {
    if (!SkillsGraphs.chartJsScriptTag) {
      SkillsGraphs.chartJsScriptTag = document.createElement("script");
      SkillsGraphs.chartJsScriptTag.src = "https://cdnjs.cloudflare.com/ajax/libs/Chart.js/3.9.1/chart.min.js";
      document.body.appendChild(SkillsGraphs.chartJsScriptTag);
    }

    while (typeof Chart === "undefined") {
      await new Promise((resolve) => setTimeout(() => resolve(true), 100));
    }
  }
}

customElements.define("skills-graphs", SkillsGraphs);
