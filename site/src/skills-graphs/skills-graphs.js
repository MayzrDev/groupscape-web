/* global Chart */
import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { SkillName } from "../data/skill";
import { GroupData, groupData } from "../data/group-data";
import { computeXpGains, rankEntries } from "../data/xp-gain";

export function formatLeaderboardValue(value) {
  return Math.round(value).toLocaleString();
}

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
      skill: "Overall",
    };
    this.fetchGeneration = 0;

    this.chartContainer = this.querySelector(".skills-graphs__chart-container");
    this.periodButtons = this.querySelectorAll(".skills-graphs__period-btn");
    this.refreshButton = this.querySelector(".skills-graphs__refresh");
    this.skillSelect = this.querySelector(".skills-graphs__skill-select");
    this.leaderboardList = this.querySelector(".skills-graphs__leaderboard-list");
    this.leaderboardEmpty = this.querySelector(".skills-graphs__leaderboard-empty");

    this.state.skill = this.skillSelect.value;

    this.periodButtons.forEach((btn) => {
      this.eventListener(btn, "click", this.handlePeriodChange.bind(this));
    });
    this.eventListener(this.refreshButton, "click", this.handleRefreshClicked.bind(this));
    this.eventListener(this.skillSelect, "change", this.handleSkillSelectChange.bind(this));

    this.triggerRefresh();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  handleSkillSelectChange() {
    this.state.skill = this.skillSelect.value;
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
  // the leaderboard render so stale in-flight responses from a previous state are discarded.
  triggerRefresh() {
    const generation = ++this.fetchGeneration;
    this.subscribeOnce("get-group-data", () => {
      this.createChart(generation);
    });
  }

  // Ranks the same skillDataForGroup/period the chart just rendered, instead of a separate
  // leaderboard API call - the leaderboard's server-side windows (daily/weekly/all_time) are
  // coarser than the chart's 7 periods, which made the sidepanel ignore 1H/6H/12H/24H and
  // 30D/1Y distinctions. Reading off the chart's own numbers keeps the two in lockstep.
  renderLeaderboardFromRawData(rawData) {
    const gains = computeXpGains(rawData, groupData, this.state.skill, this.state.period);
    const entries = rankEntries(gains.filter((entry) => entry.member_name !== "@SHARED"));
    this.renderLeaderboard(entries);
  }

  renderLeaderboard(entries) {
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
      value.textContent = formatLeaderboardValue(entry.value);
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
      const [rawData] = await Promise.all([api.getSkillData(this.state.period), this.waitForChartjs()]);
      if (generation !== this.fetchGeneration) return;

      rawData.sort((a, b) => a.name.localeCompare(b.name));

      const skillGraph = document.createElement("skill-graph");
      skillGraph.setAttribute("data-period", this.state.period);

      rawData.forEach((playerSkillData) => {
        playerSkillData.skill_data.forEach((x) => {
          x.time = new Date(x.time);
          x.data = GroupData.transformSkillsFromStorage(x.data);
        });
        playerSkillData.skill_data.sort((a, b) => b.time - a.time);
      });
      skillGraph.skillDataForGroup = rawData;
      skillGraph.setAttribute("skill-name", this.state.skill);

      overlay.remove();
      this.chartContainer.innerHTML = "";
      Chart.defaults.scale.grid.borderColor = "rgba(255, 255, 255, 0)";
      const style = getComputedStyle(document.body);
      Chart.defaults.color = style.getPropertyValue("--primary-text");
      Chart.defaults.scale.grid.color = style.getPropertyValue("--graph-grid-border");

      this.chartContainer.appendChild(skillGraph);
      this.renderLeaderboardFromRawData(rawData);
    } catch (err) {
      overlay.remove();
      console.error(err);
      this.chartContainer.innerHTML = `Failed to load ${err}`;
      this.leaderboardList.innerHTML = "";
      this.leaderboardEmpty.textContent = `Failed to load ${err}`;
      this.leaderboardEmpty.classList.add("skills-graphs__leaderboard-empty--visible");
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
