import { BaseElement } from "../base-element/base-element";

export const COMBAT_ACHIEVEMENT_TIERS = [
  ["easy", "Easy"],
  ["medium", "Medium"],
  ["hard", "Hard"],
  ["elite", "Elite"],
  ["master", "Master"],
  ["grandmaster", "Grandmaster"],
];

export class CombatAchievementMember extends BaseElement {
  constructor() {
    super();
    this.tiers = COMBAT_ACHIEVEMENT_TIERS;
  }

  html() {
    return `{{combat-achievement-member.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.head = this.querySelector(".combat-achievement-member__head");
    this.body = this.querySelector(".combat-achievement-member__body");
    this.arrow = this.querySelector(".combat-achievement-member__arrow");
    this.renderBars();
    this.renderTierRows();

    this.eventListener(this.head, "click", this.toggleOpen.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  tierComplete(tierKey) {
    return !!this.member.combatAchievements?.tiers?.[tierKey];
  }

  completedTierCount() {
    return this.tiers.filter(([key]) => this.tierComplete(key)).length;
  }

  taskCount() {
    const tasks = this.member.combatAchievements?.tasks || {};
    return Object.values(tasks).filter(Boolean).length;
  }

  renderBars() {
    const bars = this.querySelector(".combat-achievement-member__bars");
    bars.innerHTML = "";
    for (const [key] of this.tiers) {
      const bar = document.createElement("div");
      bar.className = `combat-achievement-member__bar combat-achievement-member__bar--${key}`;
      bar.classList.toggle("combat-achievement-member__bar--done", this.tierComplete(key));
      bars.appendChild(bar);
    }
  }

  renderTierRows() {
    const container = this.querySelector(".combat-achievement-member__tiers");
    container.innerHTML = "";
    for (const [key, label] of this.tiers) {
      const done = this.tierComplete(key);
      const row = document.createElement("div");
      row.className = `combat-achievement-member__tier-row combat-achievement-member__tier-row--${key}`;

      const name = document.createElement("span");
      name.className = "combat-achievement-member__tier-label";
      name.textContent = label;

      const state = document.createElement("span");
      state.className = "combat-achievement-member__tier-state";
      state.textContent = done ? "Complete" : "Locked";
      state.classList.toggle("combat-achievement-member__tier-state--done", done);

      row.appendChild(name);
      row.appendChild(state);
      container.appendChild(row);
    }
  }

  toggleOpen() {
    this.body.classList.toggle("combat-achievement-member__body--open");
    this.arrow.classList.toggle("combat-achievement-member__arrow--open");
  }
}

customElements.define("combat-achievement-member", CombatAchievementMember);
