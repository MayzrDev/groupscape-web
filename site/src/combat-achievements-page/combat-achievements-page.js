import { BaseElement } from "../base-element/base-element";
import { pubsub } from "../data/pubsub";
import { combatAchievement, COMBAT_ACHIEVEMENT_TIERS } from "../data/combat-achievement";

export class CombatAchievementsPage extends BaseElement {
  constructor() {
    super();
    this.members = [];
    this.sortKey = "tierCount";
    this.sortDir = "desc";
  }

  html() {
    return `{{combat-achievements-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.tableBody = this.querySelector(".combat-achievements-page__body");
    this.eventListener(this.querySelector("thead"), "click", this.handleHeaderClick.bind(this));
    this.eventListener(this.tableBody, "click", this.handleRowClick.bind(this));

    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));
    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  handleUpdatedMembers(members) {
    this.members = members.filter((member) => member.name !== "@SHARED");
    this.renderRows();
  }

  handleHeaderClick(event) {
    const sortKey = event.target.closest("th")?.getAttribute("sort-key");
    if (!sortKey) return;

    if (this.sortKey === sortKey) {
      this.sortDir = this.sortDir === "asc" ? "desc" : "asc";
    } else {
      this.sortKey = sortKey;
      this.sortDir = sortKey === "name" ? "asc" : "desc";
    }
    this.renderRows();
  }

  handleRowClick(event) {
    const playerName = event.target.closest("button[player-name]")?.getAttribute("player-name");
    if (!playerName) return;

    const combatAchievementsEl = document.createElement("combat-achievements");
    combatAchievementsEl.setAttribute("player-name", playerName);
    document.body.appendChild(combatAchievementsEl);
  }

  sortedMembers() {
    const direction = this.sortDir === "asc" ? 1 : -1;
    return [...this.members].sort((a, b) => {
      if (this.sortKey === "name") {
        return a.name.localeCompare(b.name) * direction;
      }
      return (combatAchievement.completedTierCount(a) - combatAchievement.completedTierCount(b)) * direction;
    });
  }

  sortArrow(sortKey) {
    if (this.sortKey !== sortKey) return "";
    return this.sortDir === "asc" ? " &#9652;" : " &#9662;";
  }

  renderRows() {
    if (!this.tableBody) return;

    this.querySelectorAll("th[sort-key]").forEach((th) => {
      th.innerHTML = `${th.getAttribute("label")}${this.sortArrow(th.getAttribute("sort-key"))}`;
    });

    this.tableBody.innerHTML = this.sortedMembers()
      .map((member) => {
        const tierCells = COMBAT_ACHIEVEMENT_TIERS.map(
          ([key]) => `
          <td>
            <span class="combat-achievements-page__pip combat-achievements-page__pip--${key} ${
            combatAchievement.isTierComplete(member, key) ? "combat-achievements-page__pip--on" : ""
          }"></span>
          </td>`
        ).join("");

        return `
          <tr>
            <td><button type="button" class="combat-achievements-page__name" player-name="${member.name}">${
          member.name
        }</button></td>
            ${tierCells}
            <td class="combat-achievements-page__count ${
              combatAchievement.completedTierCount(member) === COMBAT_ACHIEVEMENT_TIERS.length
                ? "combat-achievements-page__count--full"
                : ""
            }">${combatAchievement.completedTierCount(member)}/${COMBAT_ACHIEVEMENT_TIERS.length}</td>
          </tr>`;
      })
      .join("");
  }
}

customElements.define("combat-achievements-page", CombatAchievementsPage);
