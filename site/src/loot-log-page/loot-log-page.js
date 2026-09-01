import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { pubsub } from "../data/pubsub";
import { utility } from "../utility";
import { slugifyNpcName } from "../data/npc-slug";

const REFRESH_INTERVAL_MS = 15000;

export class LootLogPage extends BaseElement {
  constructor() {
    super();
    this.rows = [];
    this.sources = [];
    this.members = [];
    this.bosses = [];
    this.selectedMember = "";
    this.selectedBoss = "";
    this.timeWindow = "1h";
  }

  html() {
    return `{{loot-log-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.summaryContainer = this.querySelector(".loot-log-page__summary");
    this.timeSelect = this.querySelector(".loot-log-page__time-select");
    this.customSince = this.querySelector(".loot-log-page__custom-since");
    this.customUntil = this.querySelector(".loot-log-page__custom-until");
    this.bossSelect = this.querySelector(".loot-log-page__boss-select");
    this.memberSelect = this.querySelector(".loot-log-page__member-select");
    this.list = this.querySelector(".loot-log-page__list");
    this.empty = this.querySelector(".loot-log-page__empty");

    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));
    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }

    this.eventListener(this.memberSelect, "change", () => {
      this.selectedMember = this.memberSelect.value;
      this.fetchLoot();
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
    const summary = await api.getLootSummary(scope);
    this.rows = summary.rows;
    this.sources = summary.sources;
    this.renderBossOptions();
    this.renderList();
    this.renderSummary();
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

  renderSummary() {
    if (!this.summaryContainer) return;
    if (this.rows.length === 0) {
      this.summaryContainer.innerHTML = "";
      return;
    }
    const totalValue = this.rows.reduce((sum, row) => sum + row.total_value, 0);
    const eventCount = this.sources.reduce((sum, source) => sum + source.event_count, 0);
    this.summaryContainer.innerHTML = `
      <div class="loot-log-page__summary-stats">
        <div class="loot-log-page__summary-card">
          <div class="loot-log-page__summary-n">${totalValue.toLocaleString()}</div>
          <div class="loot-log-page__summary-l">Total Value</div>
        </div>
        <div class="loot-log-page__summary-card">
          <div class="loot-log-page__summary-n">${eventCount.toLocaleString()}</div>
          <div class="loot-log-page__summary-l">Events</div>
        </div>
        <div class="loot-log-page__summary-card loot-log-page__summary-card--session">
          <div class="loot-log-page__summary-n">${this.sessionDuration()}</div>
          <div class="loot-log-page__summary-l">${this.sessionRange()}</div>
        </div>
      </div>
    `;
  }

  // First kill -> last kill across the current scope's rows, not a detected play-session
  // boundary - just the span the selected time window's activity actually covers.
  sessionBounds() {
    let first = Infinity;
    let last = -Infinity;
    for (const row of this.rows) {
      const time = new Date(row.occurred_at).getTime();
      if (time < first) first = time;
      if (time > last) last = time;
    }
    return { first: new Date(first), last: new Date(last) };
  }

  sessionDuration() {
    const { first, last } = this.sessionBounds();
    const minutes = Math.max(0, Math.round((last - first) / 60000));
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const remainingMinutes = minutes % 60;
    if (hours < 24) return `${hours}h ${remainingMinutes}m`;
    const days = Math.floor(hours / 24);
    return `${days}d ${hours % 24}h`;
  }

  sessionRange() {
    const { first, last } = this.sessionBounds();
    const sameDay = first.toDateString() === last.toDateString();
    const timeFormat = { hour: "numeric", minute: "2-digit" };
    if (sameDay) {
      return `${first.toLocaleTimeString([], timeFormat)} &ndash; ${last.toLocaleTimeString([], timeFormat)}`;
    }
    const dateTimeFormat = { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" };
    return `${first.toLocaleString([], dateTimeFormat)} &ndash; ${last.toLocaleString([], dateTimeFormat)}`;
  }

  // Rows arrive newest-first from the server, so grouping by first appearance in `this.rows`
  // preserves that order at the group level too - most-recently-active source first.
  groupedRows() {
    const sourceCounts = new Map();
    for (const source of this.sources) {
      sourceCounts.set(this.sourceKey(source.source_name, source.source_type, source.clue_tier), source.event_count);
    }

    const groups = new Map();
    const order = [];
    for (const row of this.rows) {
      const key = this.sourceKey(row.source_name, row.source_type, row.clue_tier);
      if (!groups.has(key)) {
        groups.set(key, {
          sourceName: row.source_name,
          sourceType: row.source_type,
          clueTier: row.clue_tier,
          eventCount: sourceCounts.get(key) || 0,
          showKillers: !this.selectedMember,
          rows: [],
        });
        order.push(key);
      }
      groups.get(key).rows.push(row);
    }
    return order.map((key) => groups.get(key));
  }

  sourceKey(sourceName, sourceType, clueTier) {
    return `${sourceName} ${sourceType} ${clueTier || ""}`;
  }

  renderList() {
    if (!this.list) return;
    this.list.innerHTML = "";
    this.empty.classList.toggle("loot-log-page__empty--visible", this.rows.length === 0);

    for (const group of this.groupedRows()) {
      const groupElement = document.createElement("loot-log-group");
      groupElement.group = group;
      this.list.appendChild(groupElement);
    }
  }
}

customElements.define("loot-log-page", LootLogPage);
