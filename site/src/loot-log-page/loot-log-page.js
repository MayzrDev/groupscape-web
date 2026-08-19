import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { pubsub } from "../data/pubsub";
import { utility } from "../utility";

const REFRESH_INTERVAL_MS = 15000;

export class LootLogPage extends BaseElement {
  constructor() {
    super();
    this.rows = [];
    this.split = null;
    this.members = [];
    this.selectedMember = "";
    this.sort = "value";
  }

  html() {
    return `{{loot-log-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.splitContainer = this.querySelector(".loot-log-page__split");
    this.memberSelect = this.querySelector(".loot-log-page__member-select");
    this.sortSelect = this.querySelector(".loot-log-page__sort-select");
    this.list = this.querySelector(".loot-log-page__list");
    this.empty = this.querySelector(".loot-log-page__empty");

    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));
    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }

    this.eventListener(this.memberSelect, "change", () => {
      this.selectedMember = this.memberSelect.value;
      this.fetchLootSummary();
    });
    this.eventListener(this.sortSelect, "change", () => {
      this.sort = this.sortSelect.value;
      this.fetchLootSummary();
    });

    this.fetchLootSummary();
    this.fetchLootSplit();
    this.refreshInterval = utility.callOnInterval(
      () => {
        this.fetchLootSummary();
        this.fetchLootSplit();
      },
      REFRESH_INTERVAL_MS,
      false
    );
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    if (this.refreshInterval) window.clearInterval(this.refreshInterval);
  }

  handleUpdatedMembers(members) {
    this.members = members.filter((member) => member.name !== "@SHARED");
    this.renderMemberOptions();
  }

  renderMemberOptions() {
    if (!this.memberSelect) return;
    const previousValue = this.memberSelect.value || this.selectedMember;
    this.memberSelect.innerHTML = `
      <option value="">All members</option>
      ${this.members.map((member) => `<option value="${member.name}">${member.name}</option>`).join("")}
    `;
    this.memberSelect.value = previousValue;
  }

  async fetchLootSummary() {
    this.rows = await api.getLootSummary({ memberName: this.selectedMember || undefined, sort: this.sort });
    this.renderList();
  }

  async fetchLootSplit() {
    this.split = await api.getLootSplit();
    this.renderSplit();
  }

  renderSplit() {
    if (!this.splitContainer) return;
    if (!this.split) {
      this.splitContainer.innerHTML = "";
      return;
    }
    this.splitContainer.innerHTML = `
      <div class="loot-log-page__split-stats">
        <div class="loot-log-page__split-card">
          <div class="loot-log-page__split-n">${this.split.total_value.toLocaleString()}</div>
          <div class="loot-log-page__split-l">Total Value</div>
        </div>
        <div class="loot-log-page__split-card">
          <div class="loot-log-page__split-n">${this.split.kill_count.toLocaleString()}</div>
          <div class="loot-log-page__split-l">Kills</div>
        </div>
        <div class="loot-log-page__split-card">
          <div class="loot-log-page__split-n">${this.split.per_person_gp.toLocaleString()}</div>
          <div class="loot-log-page__split-l">Per Person</div>
        </div>
      </div>
      <div class="loot-log-page__participants">
        ${this.split.participants
          .map(
            (participant) => `
          <div class="loot-log-page__participant-chip">
            ${participant.member_name} — <b>${
              participant.kill_count
            }</b> kills · ${participant.loot_value.toLocaleString()}
          </div>
        `
          )
          .join("")}
      </div>
    `;
  }

  renderList() {
    if (!this.list) return;
    this.list.innerHTML = "";
    this.empty.classList.toggle("loot-log-page__empty--visible", this.rows.length === 0);

    for (const row of this.rows) {
      const rowElement = document.createElement("loot-log-row");
      rowElement.row = row;
      this.list.appendChild(rowElement);
    }
  }
}

customElements.define("loot-log-page", LootLogPage);
