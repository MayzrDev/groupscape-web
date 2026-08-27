import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { pubsub } from "../data/pubsub";
import { utility } from "../utility";

const REFRESH_INTERVAL_MS = 15000;

// Mirrors the server's slugify_npc_name (server/src/drop_rates.rs) so the fallback boss list
// (built from raw kill npc_name values when the curated boss list hasn't loaded yet) sends the
// same slug the backend filters on.
function slugifyNpcName(name) {
  return name
    .replace(/'/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

export class LootLogPage extends BaseElement {
  constructor() {
    super();
    this.rows = [];
    this.split = null;
    this.members = [];
    this.bosses = [];
    this.selectedMember = "";
    this.selectedBoss = "";
    this.selectedClueTier = "";
    this.timeWindow = "1h";
    this.splitMode = "reported";
    this.sort = "value";
  }

  html() {
    return `{{loot-log-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.splitContainer = this.querySelector(".loot-log-page__split");
    this.timeSelect = this.querySelector(".loot-log-page__time-select");
    this.customSince = this.querySelector(".loot-log-page__custom-since");
    this.customUntil = this.querySelector(".loot-log-page__custom-until");
    this.bossSelect = this.querySelector(".loot-log-page__boss-select");
    this.clueTierSelect = this.querySelector(".loot-log-page__clue-tier-select");
    this.memberSelect = this.querySelector(".loot-log-page__member-select");
    this.sortSelect = this.querySelector(".loot-log-page__sort-select");
    this.splitModeSelect = this.querySelector(".loot-log-page__split-mode-select");
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
    this.eventListener(this.timeSelect, "change", () => {
      this.timeWindow = this.timeSelect.value;
      const custom = this.timeWindow === "custom";
      this.customSince.hidden = !custom;
      this.customUntil.hidden = !custom;
      this.fetchLoot();
    });
    this.eventListener(this.customSince, "change", () => this.fetchLoot());
    this.eventListener(this.customUntil, "change", () => this.fetchLoot());
    this.eventListener(this.bossSelect, "change", () => {
      this.selectedBoss = this.bossSelect.value;
      this.fetchLoot();
    });
    this.eventListener(this.clueTierSelect, "change", () => {
      this.selectedClueTier = this.clueTierSelect.value;
      this.fetchLoot();
    });
    this.eventListener(this.splitModeSelect, "change", () => {
      this.splitMode = this.splitModeSelect.value;
      this.fetchLoot();
    });
    this.eventListener(this.sortSelect, "change", () => {
      this.sort = this.sortSelect.value;
      this.fetchLootSummary();
    });

    Promise.all([api.getLootBosses()]).then(([bosses]) => {
      this.bosses = bosses;
      this.renderBossOptions();
    });
    this.fetchLoot();
    this.refreshInterval = utility.callOnInterval(
      () => {
        this.fetchLoot();
      },
      REFRESH_INTERVAL_MS,
      false
    );
  }

  getScope() {
    const scope = {
      memberName: this.selectedMember || undefined,
      boss: this.selectedBoss || undefined,
      clueTier: this.selectedClueTier || undefined,
      splitMode: this.splitMode,
    };
    if (this.timeWindow !== "all") {
      if (this.timeWindow === "custom") {
        if (this.customSince.value) scope.since = new Date(`${this.customSince.value}T00:00:00`).toISOString();
        if (this.customUntil.value) scope.until = new Date(`${this.customUntil.value}T23:59:59.999`).toISOString();
        return scope;
      }
      const units = { "30m": 30, "1h": 60, "2h": 120, "4h": 240, "12h": 720, "1d": 1440, "7d": 10080, "30d": 43200 };
      scope.since = new Date(Date.now() - units[this.timeWindow] * 60000).toISOString();
    }
    return scope;
  }

  async fetchLoot() {
    const scope = this.getScope();
    this.rows = await api.getLootSummary({ ...scope, sort: this.sort });
    this.split = await api.getLootSplit(scope);
    this.renderBossOptions();
    this.renderList();
    this.renderSplit();
  }

  renderBossOptions() {
    const sources = this.bosses.length
      ? this.bosses
      : [...new Set(this.rows.filter((row) => row.source_type !== "clue").map((row) => row.source_name))]
          .sort()
          .map((name) => ({ slug: slugifyNpcName(name), name, source_type: "kill" }));
    const current = this.selectedBoss;
    const bossOptions = sources.filter((source) => source.source_type === "kill");
    const chestOptions = sources.filter((source) => source.source_type === "chest");
    const optgroup = (label, options) =>
      options.length
        ? `<optgroup label="${label}">${options
            .map((source) => `<option value="${source.slug}">${source.name}</option>`)
            .join("")}</optgroup>`
        : "";
    this.bossSelect.innerHTML = `<option value="">All sources</option>${optgroup("Bosses", bossOptions)}${optgroup(
      "Chests & Minigames",
      chestOptions
    )}`;
    this.bossSelect.value = sources.some((source) => source.slug === current) ? current : "";
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
    await this.fetchLoot();
  }

  async fetchLootSplit() {
    await this.fetchLoot();
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
          <div class="loot-log-page__split-n">${this.split.event_count.toLocaleString()}</div>
          <div class="loot-log-page__split-l">Events</div>
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
              participant.event_count
            }</b> events · ${participant.loot_value.toLocaleString()}
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
