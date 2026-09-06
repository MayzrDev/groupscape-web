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
    this.cardsBody = this.querySelector(".combat-achievements-page__cards");
    this.groupPointsValue = this.querySelector(".combat-achievements-page__group-points-value");
    this.eventListener(this.querySelector("thead"), "click", this.handleHeaderClick.bind(this));
    this.eventListener(
      this.querySelector(".combat-achievements-page__sort-row"),
      "click",
      this.handleSortChipClick.bind(this)
    );
    this.eventListener(this.tableBody, "click", this.handleRowClick.bind(this));
    this.eventListener(this.cardsBody, "click", this.handleRowClick.bind(this));

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
    this.applySortKey(sortKey);
  }

  handleSortChipClick(event) {
    const sortKey = event.target.closest("[sort-key]")?.getAttribute("sort-key");
    if (!sortKey) return;
    this.applySortKey(sortKey);
  }

  applySortKey(sortKey) {
    if (this.sortKey === sortKey) {
      this.sortDir = this.sortDir === "asc" ? "desc" : "asc";
    } else {
      this.sortKey = sortKey;
      this.sortDir = sortKey === "name" ? "asc" : "desc";
    }
    this.renderRows();
  }

  handleRowClick(event) {
    const row = event.target.closest("[player-name]");
    const playerName = row?.getAttribute("player-name");
    if (!playerName) return;

    const tierKey = event.target.closest("[tier-key]")?.getAttribute("tier-key");

    const combatAchievementsEl = document.createElement("combat-achievements");
    combatAchievementsEl.setAttribute("player-name", playerName);
    if (tierKey) combatAchievementsEl.setAttribute("open-tier", tierKey);
    document.body.appendChild(combatAchievementsEl);
  }

  sortedMembers() {
    const direction = this.sortDir === "asc" ? 1 : -1;
    return [...this.members].sort((a, b) => {
      if (this.sortKey === "name") {
        return a.name.localeCompare(b.name) * direction;
      }
      if (this.sortKey === "points") {
        return (combatAchievement.totalPoints(a) - combatAchievement.totalPoints(b)) * direction;
      }
      return (combatAchievement.completedTierCount(a) - combatAchievement.completedTierCount(b)) * direction;
    });
  }

  sortArrow(sortKey) {
    if (this.sortKey !== sortKey) return "";
    return this.sortDir === "asc" ? " &#9652;" : " &#9662;";
  }

  memberTierData(member, key) {
    return {
      key,
      done: combatAchievement.completedTaskCountForTier(member, key),
      total: combatAchievement.totalTasksForTier(key),
      percent: combatAchievement.tierCompletionPercent(member, key),
      complete: combatAchievement.isTierComplete(member, key),
    };
  }

  groupTierData(members, key) {
    const total = combatAchievement.totalTasksForTier(key);
    const summedDone = members.reduce(
      (sum, member) => sum + combatAchievement.completedTaskCountForTier(member, key),
      0
    );
    const done = members.length ? Math.round(summedDone / members.length) : 0;
    const percent = total && members.length ? Math.round((summedDone / (total * members.length)) * 100) : 0;
    return { key, done, total, percent, complete: false };
  }

  tierBarCell({ key, done, total, percent, complete, group = false }) {
    const trackClasses = [
      "combat-achievements-page__bar-track",
      group ? "combat-achievements-page__bar-track--group" : "",
      complete ? "combat-achievements-page__bar-track--complete" : "",
    ]
      .filter(Boolean)
      .join(" ");

    const fracClass = group
      ? "combat-achievements-page__bar-frac combat-achievements-page__bar-frac--group"
      : "combat-achievements-page__bar-frac";

    const inner = `
      <div class="${trackClasses}">
        <span class="combat-achievements-page__bar-fill combat-achievements-page__bar-fill--${key}" style="width: ${percent}%"></span>
      </div>
      <div class="${fracClass}">
        <span>${done}/${total}</span>
        <span class="combat-achievements-page__bar-pct">${percent}%</span>
      </div>`;

    if (group) return `<td>${inner}</td>`;

    return `
      <td>
        <div class="combat-achievements-page__barcell" tier-key="${key}">
          ${inner}
        </div>
      </td>`;
  }

  tierCardBlock({ key, done, total, percent, complete }) {
    const trackClasses = [
      "combat-achievements-page__card-tier-track",
      complete ? "combat-achievements-page__card-tier-track--complete" : "",
    ]
      .filter(Boolean)
      .join(" ");

    return `
      <div class="combat-achievements-page__card-tier combat-achievements-page__card-tier--${key}" tier-key="${key}">
        <div class="combat-achievements-page__card-tier-label">
          <span>${combatAchievement.tierLabel(key)}</span>
          <span class="combat-achievements-page__card-tier-frac">${done}/${total}</span>
        </div>
        <div class="${trackClasses}">
          <span class="combat-achievements-page__card-tier-fill" style="width: ${percent}%"></span>
        </div>
      </div>`;
  }

  renderGroupRow(members) {
    if (members.length < 2) return "";

    const tierCells = COMBAT_ACHIEVEMENT_TIERS.map(([key]) =>
      this.tierBarCell({ ...this.groupTierData(members, key), group: true })
    ).join("");

    return `
      <tr class="combat-achievements-page__row--group">
        <td>
          <div class="combat-achievements-page__member-cell">
            <div class="combat-achievements-page__group-icon">&#9878;</div>
            <div>
              <span class="combat-achievements-page__group-label">Group average</span>
              <span class="combat-achievements-page__group-sub">${members.length} members</span>
            </div>
          </div>
        </td>
        ${tierCells}
        <td>&mdash;</td>
        <td>&mdash;</td>
      </tr>`;
  }

  renderGroupCard(members) {
    if (members.length < 2) return "";

    const tierBlocks = COMBAT_ACHIEVEMENT_TIERS.map(([key]) =>
      this.tierCardBlock(this.groupTierData(members, key))
    ).join("");

    return `
      <div class="combat-achievements-page__card combat-achievements-page__card--group">
        <div class="combat-achievements-page__card-head">
          <div class="combat-achievements-page__group-icon">&#9878;</div>
          <div class="combat-achievements-page__card-id">
            <span class="combat-achievements-page__group-label">Group average</span>
            <span class="combat-achievements-page__group-sub">${members.length} members</span>
          </div>
        </div>
        <div class="combat-achievements-page__card-tiers">${tierBlocks}</div>
      </div>`;
  }

  renderRows() {
    if (!this.tableBody) return;

    this.querySelectorAll("th[sort-key]").forEach((th) => {
      th.innerHTML = `${th.getAttribute("label")}${this.sortArrow(th.getAttribute("sort-key"))}`;
    });
    this.querySelectorAll(".combat-achievements-page__sort-chip").forEach((chip) => {
      const sortKey = chip.getAttribute("sort-key");
      chip.classList.toggle("combat-achievements-page__sort-chip--active", sortKey === this.sortKey);
      chip.textContent = `${chip.getAttribute("label")}${sortKey === this.sortKey ? this.sortArrow(sortKey) : ""}`;
    });

    const sorted = this.sortedMembers();
    const pointsByMember = sorted.map((member) => combatAchievement.totalPoints(member));
    const topPoints = Math.max(0, ...pointsByMember);
    const secondPoints = Math.max(0, ...pointsByMember.filter((points) => points !== topPoints));

    const groupRowHtml = this.renderGroupRow(sorted);
    const groupCardHtml = this.renderGroupCard(sorted);
    if (this.groupPointsValue) {
      this.groupPointsValue.textContent = pointsByMember.reduce((sum, points) => sum + points, 0);
    }

    let memberRowsHtml = "";
    let memberCardsHtml = "";

    sorted.forEach((member) => {
      const tierData = COMBAT_ACHIEVEMENT_TIERS.map(([key]) => this.memberTierData(member, key));
      const tierCells = tierData.map((data) => this.tierBarCell(data)).join("");
      const tierBlocks = tierData.map((data) => this.tierCardBlock(data)).join("");

      const points = combatAchievement.totalPoints(member);
      const tierCount = combatAchievement.completedTierCount(member);
      const isTop = points === topPoints && points > 0;
      const isSecond = points === secondPoints && points > 0;
      const rankClass = isTop
        ? "combat-achievements-page__name--rank1"
        : isSecond
        ? "combat-achievements-page__name--rank2"
        : "";
      const fullClass =
        tierCount === COMBAT_ACHIEVEMENT_TIERS.length ? "combat-achievements-page__count-pill--full" : "";
      const topStar = isTop ? '<span class="combat-achievements-page__points-rank">&#9733; top scorer</span>' : "";

      memberRowsHtml += `
        <tr class="combat-achievements-page__row--member" player-name="${member.name}">
          <td>
            <div class="combat-achievements-page__member-cell">
              <player-icon player-name="${member.name}"></player-icon>
              <span class="combat-achievements-page__name ${rankClass}">${member.name}</span>
            </div>
          </td>
          ${tierCells}
          <td>
            <span class="combat-achievements-page__count-pill ${fullClass}">${tierCount}/${
        COMBAT_ACHIEVEMENT_TIERS.length
      }</span>
          </td>
          <td>
            <span class="combat-achievements-page__points ${
              isTop ? "combat-achievements-page__points--top" : ""
            }">${points}${topStar}</span>
          </td>
        </tr>`;

      memberCardsHtml += `
        <div class="combat-achievements-page__card" player-name="${member.name}">
          <div class="combat-achievements-page__card-head">
            <player-icon player-name="${member.name}"></player-icon>
            <div class="combat-achievements-page__card-id">
              <span class="combat-achievements-page__name ${rankClass}">${member.name}</span>
              <span class="combat-achievements-page__card-sub">${tierCount}/${
        COMBAT_ACHIEVEMENT_TIERS.length
      } tiers</span>
            </div>
            <div class="combat-achievements-page__card-stats">
              <span class="combat-achievements-page__points ${
                isTop ? "combat-achievements-page__points--top" : ""
              }">${points}</span>
              ${isTop ? topStar : ""}
              <span class="combat-achievements-page__count-pill ${fullClass}">${tierCount}/${
        COMBAT_ACHIEVEMENT_TIERS.length
      }</span>
            </div>
          </div>
          <div class="combat-achievements-page__card-tiers">${tierBlocks}</div>
        </div>`;
    });

    this.tableBody.innerHTML = groupRowHtml + memberRowsHtml;
    if (this.cardsBody) {
      this.cardsBody.innerHTML = groupCardHtml + memberCardsHtml;
    }
  }
}

customElements.define("combat-achievements-page", CombatAchievementsPage);
