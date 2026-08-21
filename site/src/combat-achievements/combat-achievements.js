import { BaseElement } from "../base-element/base-element";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";
// eslint-disable-next-line no-unused-vars
import { combatAchievement, COMBAT_ACHIEVEMENT_TIERS } from "../data/combat-achievement";
import { COMBAT_ACHIEVEMENT_BOSS_LEVELS } from "../data/combat-achievement-boss-levels";

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
    this.view = "tier";
    this.searchTerm = "";
    this.expandedBoss = null;
    loadingScreenManager.hideLoadingScreen();

    this.render();
    this.bindElements();
  }

  bindElements() {
    this.tiers = this.querySelector(".combat-achievements__tiers");
    this.accordionBody = this.querySelector(".combat-achievements__accordion-body");
    this.bossView = this.querySelector(".combat-achievements__boss-view");
    this.viewSwitch = this.querySelector(".combat-achievements__view-switch");
    this.searchInput = this.querySelector(".combat-achievements__search-input");
    this.background = this.querySelector(".dialog__visible");

    this.renderAccordionBody();
    this.renderBossView();

    this.eventListener(this.tiers, "click", this.handleTierClick.bind(this));
    this.eventListener(this.viewSwitch, "click", this.handleViewClick.bind(this));
    this.eventListener(this.searchInput, "input", this.handleSearchInput.bind(this));
    this.eventListener(this.bossView, "click", this.handleBossCardClick.bind(this));
    this.eventListener(this.bossView, "keydown", this.handleBossCardKeydown.bind(this));
    this.eventListener(this.background, "click", this.closeIfBackgroundClick.bind(this));
    this.eventListener(this.querySelector(".dialog__close"), "click", this.close.bind(this));
  }

  nextUnlockLabel() {
    const next = combatAchievement.nextUnlock(this.member);
    return next ? `Next unlock in ${next.pointsRemaining} points` : "All reward tiers unlocked";
  }

  pointsFillPercent() {
    const next = combatAchievement.nextUnlock(this.member);
    if (!next) return 100;
    const earned = combatAchievement.totalPoints(this.member);
    const target = earned + next.pointsRemaining;
    return target ? Math.round((earned / target) * 100) : 0;
  }

  handleTierClick(event) {
    const button = event.target.closest("button[tier-key]");
    const tierKey = button?.getAttribute("tier-key");
    if (!tierKey || tierKey === this.openTier) return;

    this.tiers.querySelectorAll("button[tier-key]").forEach((btn) => {
      btn.classList.toggle("combat-achievements__tier-card--open", btn.getAttribute("tier-key") === tierKey);
    });

    this.openTier = tierKey;
    this.renderAccordionBody();
  }

  handleViewClick(event) {
    const button = event.target.closest("button[view]");
    const view = button?.getAttribute("view");
    if (!view || view === this.view) return;

    this.view = view;
    this.viewSwitch.querySelectorAll("button[view]").forEach((btn) => {
      btn.classList.toggle("combat-achievements__view-btn--active", btn.getAttribute("view") === view);
    });

    this.querySelector(".combat-achievements__tier-view").classList.toggle(
      "combat-achievements__hidden",
      view !== "tier"
    );
    this.bossView.classList.toggle("combat-achievements__hidden", view !== "boss");
  }

  handleBossCardClick(event) {
    if (event.target.closest("a")) return;

    const card = event.target.closest("[boss-name]");
    const bossName = card?.getAttribute("boss-name");
    if (!bossName) return;

    this.expandedBoss = this.expandedBoss === bossName ? null : bossName;
    this.renderBossView();
  }

  handleBossCardKeydown(event) {
    if (event.key !== "Enter" && event.key !== " ") return;
    if (!event.target.closest("[boss-name]")) return;

    event.preventDefault();
    this.handleBossCardClick(event);
  }

  handleSearchInput(event) {
    this.searchTerm = event.target.value.trim().toLowerCase();
    this.renderAccordionBody();
    this.renderBossView();
  }

  renderAccordionBody() {
    const term = this.searchTerm;
    const groups = combatAchievement
      .bossGroupsForTier(this.openTier)
      .map((group) => ({
        ...group,
        tasks: group.tasks.filter(
          (task) => !term || group.boss.toLowerCase().includes(term) || task.name.toLowerCase().includes(term)
        ),
      }))
      .filter((group) => group.tasks.length > 0);

    this.accordionBody.innerHTML = `
      <div class="combat-achievements__tier-summary">
        <span>${combatAchievement.tierLabel(this.openTier)} tier</span>
        <span>${combatAchievement.completedTaskCountForTier(
          this.member,
          this.openTier
        )}/${combatAchievement.totalTasksForTier(this.openTier)} tasks complete</span>
      </div>
      ${
        groups.length
          ? groups.map((group) => this.renderBossGroup(group)).join("")
          : `<div class="combat-achievements__empty">No matching achievements</div>`
      }
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
      <a
        href="${combatAchievement.taskWikiUrl(task.name)}"
        target="_blank"
        rel="noopener"
        class="combat-achievements__task-row ${done ? "combat-achievements__task-row--done" : ""}"${title}
      >
        <span class="combat-achievements__task-check"></span>${task.name}
        <span class="combat-achievements__task-wiki-mark">&#8599;</span>
      </a>
    `;
  }

  renderBossView() {
    const term = this.searchTerm;
    const groups = combatAchievement.allBossGroups().filter((group) => {
      if (!term) return true;
      if (group.boss.toLowerCase().includes(term)) return true;
      return group.tasks.some((task) => task.name.toLowerCase().includes(term));
    });

    this.bossView.innerHTML = groups.length
      ? groups.map((group) => this.renderBossCard(group)).join("")
      : `<div class="combat-achievements__empty">No matching bosses</div>`;
  }

  renderBossCard(group) {
    const done = combatAchievement.completedTaskCountForBoss(this.member, group.tasks);
    const total = group.tasks.length;
    const percent = total ? Math.round((done / total) * 100) : 0;
    const level = COMBAT_ACHIEVEMENT_BOSS_LEVELS[group.boss];
    const expanded = this.expandedBoss === group.boss;

    return `
      <div
        role="button"
        tabindex="0"
        boss-name="${escapeAttribute(group.boss)}"
        class="combat-achievements__boss-card ${expanded ? "combat-achievements__boss-card--open" : ""}"
      >
        <div class="combat-achievements__boss-card-head">
          <span class="combat-achievements__boss-card-name">${group.boss}</span>
          <a
            href="${combatAchievement.bossWikiUrl(group.boss)}"
            target="_blank"
            rel="noopener"
            class="combat-achievements__wiki-link"
          >Wiki &#8599;</a>
        </div>
        ${level ? `<span class="combat-achievements__boss-card-level">Level ${level}</span>` : ""}
        <span class="combat-achievements__boss-card-track"><span class="combat-achievements__boss-card-fill" style="width: ${percent}%"></span></span>
        <span class="combat-achievements__boss-card-frac">${done}/${total}</span>
        ${
          expanded
            ? `<div class="combat-achievements__task-grid combat-achievements__task-grid--boss">
                ${group.tasks.map((task) => this.renderTaskRow(task)).join("")}
              </div>`
            : ""
        }
      </div>
    `;
  }
}

customElements.define("combat-achievements", CombatAchievements);
