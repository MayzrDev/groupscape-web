import { BaseElement } from "../base-element/base-element";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";
// eslint-disable-next-line no-unused-vars
import { combatAchievement, COMBAT_ACHIEVEMENT_TIERS } from "../data/combat-achievement";

function escapeAttribute(value) {
  return `${value}`.replace(/&/g, "&amp;").replace(/"/g, "&quot;");
}

export class CombatAchievements extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{combat-achievements.html}}`;
  }

  async connectedCallback() {
    super.connectedCallback();
    loadingScreenManager.showLoadingScreen();
    this.playerName = this.getAttribute("player-name");
    this.subscribeOnce("get-group-data", this.init.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    loadingScreenManager.hideLoadingScreen();
  }

  closeIfBackgroundClick(evt) {
    if (evt.target === this.background) {
      this.close();
    }
  }

  close() {
    this.remove();
  }

  async init(groupData) {
    await combatAchievement.initCatalog();
    this.member = groupData.members.get(this.playerName);
    this.openTier = combatAchievement.firstIncompleteTier(this.member);
    loadingScreenManager.hideLoadingScreen();

    this.render();

    this.medallions = this.querySelector(".combat-achievements__medallions");
    this.accordionBody = this.querySelector(".combat-achievements__accordion-body");
    this.background = this.querySelector(".dialog__visible");
    this.renderAccordionBody();

    this.eventListener(this.medallions, "click", this.handleMedallionClick.bind(this));
    this.eventListener(this.background, "click", this.closeIfBackgroundClick.bind(this));
    this.eventListener(this.querySelector(".dialog__close"), "click", this.close.bind(this));
  }

  handleMedallionClick(event) {
    const button = event.target.closest("button[tier-key]");
    const tierKey = button?.getAttribute("tier-key");
    if (!tierKey || tierKey === this.openTier) return;

    this.medallions.querySelectorAll("button[tier-key]").forEach((btn) => {
      btn.classList.toggle("combat-achievements__medallion--open", btn.getAttribute("tier-key") === tierKey);
    });

    this.openTier = tierKey;
    this.renderAccordionBody();
  }

  renderAccordionBody() {
    const groups = combatAchievement.bossGroupsForTier(this.openTier);
    this.accordionBody.innerHTML = `
      <div class="combat-achievements__tier-summary">
        <span>${combatAchievement.tierLabel(this.openTier)} tier</span>
        <span>${combatAchievement.completedTaskCountForTier(
          this.member,
          this.openTier
        )}/${combatAchievement.totalTasksForTier(this.openTier)} tasks complete</span>
      </div>
      ${groups.map((group) => this.renderBossGroup(group)).join("")}
    `;
  }

  renderBossGroup(group) {
    const doneCount = group.tasks.filter((task) => combatAchievement.isTaskComplete(this.member, task.id)).length;
    const wikiLink =
      group.boss !== "General"
        ? `<a href="${combatAchievement.bossWikiUrl(
            group.boss
          )}" target="_blank" rel="noopener" class="combat-achievements__wiki-link">Wiki &#8599;</a>`
        : "";

    return `
      <div class="combat-achievements__boss-group">
        <div class="combat-achievements__boss-head">
          <span class="combat-achievements__boss-name">${group.boss}</span>
          <span class="combat-achievements__boss-count">${doneCount}/${group.tasks.length}</span>
          ${wikiLink}
        </div>
        <div class="combat-achievements__task-grid">
          ${group.tasks.map((task) => this.renderTaskRow(task)).join("")}
        </div>
      </div>
    `;
  }

  renderTaskRow(task) {
    const done = combatAchievement.isTaskComplete(this.member, task.id);
    const title = task.description ? ` title="${escapeAttribute(task.description)}"` : "";
    return `
      <div class="combat-achievements__task-row ${done ? "combat-achievements__task-row--done" : ""}"${title}>
        <span class="combat-achievements__task-check"></span>${task.name}
      </div>
    `;
  }
}

customElements.define("combat-achievements", CombatAchievements);
