import { BaseElement } from "../base-element/base-element";
import { Prayer } from "../data/prayer";

export class PlayerStats extends BaseElement {
  constructor() {
    super();
    this.hitpoints = { current: 1, max: 1 };
    this.prayer = { current: 1, max: 1 };
    this.energy = { current: 1, max: 1 };
    this.world = 301;
    this.presenceStaleTimeout = 30 * 1000;
    this.activePrayers = [];
  }

  html() {
    return `{{player-stats.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.playerName = this.getAttribute("player-name");
    this.render();
    this.worldEl = this.querySelector(".player-stats__world");

    this.hitpointsBar = this.querySelector(".player-stats__hitpoints-bar");
    this.prayerBar = this.querySelector(".player-stats__prayer-bar");
    this.energyBar = this.querySelector(".player-stats__energy-bar");
    this.presenceEl = this.querySelector(".player-stats__presence");
    this.prayerSlots = this.querySelector(".player-stats__prayer-slots");

    this.subscribe(`stats:${this.playerName}`, this.handleUpdatedStats.bind(this));
    this.subscribe(`inactive:${this.playerName}`, this.handleWentInactive.bind(this));
    this.subscribe(`active:${this.playerName}`, this.handleWentActive.bind(this));
    this.subscribe(`richPresence:${this.playerName}`, this.handleRichPresence.bind(this));
    this.subscribe(`activePrayers:${this.playerName}`, this.handleActivePrayers.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    window.clearTimeout(this.presenceHideTimeout);
  }

  handleRichPresence(text) {
    if (!text) return;
    window.clearTimeout(this.presenceHideTimeout);
    this.presenceEl.textContent = text;
    this.presenceEl.classList.add("player-stats__presence--visible");
    this.presenceHideTimeout = window.setTimeout(() => {
      this.presenceEl.classList.remove("player-stats__presence--visible");
    }, this.presenceStaleTimeout);
  }

  handleUpdatedStats(stats, member) {
    this.updateStatBars(stats);
    this.updateWorld(stats.world, member.inactive, member.lastUpdated);
  }

  handleWentInactive(inactive, member) {
    this.updateWorld(undefined, inactive, member.lastUpdated);
  }

  handleWentActive(_, member) {
    this.world = undefined;
    this.updateWorld(member.stats?.world, false);
  }

  updateWorld(world, isInactive, lastUpdated) {
    if (isInactive) {
      const locale = Intl?.DateTimeFormat()?.resolvedOptions()?.locale || undefined;
      this.worldEl.innerText = `${lastUpdated.toLocaleString(locale)}`;
      if (!this.classList.contains("player-stats__inactive")) {
        this.classList.add("player-stats__inactive");
      }
    } else if (this.world !== world) {
      this.world = world;
      if (this.classList.contains("player-stats__inactive")) {
        this.classList.remove("player-stats__inactive");
      }
      this.worldEl.innerText = `W${this.world}`;
    }
  }

  updateStatBars(stats) {
    if (stats.hitpoints === undefined || stats.prayer === undefined || stats.energy === undefined) {
      return;
    }

    this.updateText(stats.hitpoints, "hitpoints");
    this.updateText(stats.prayer, "prayer");

    window.requestAnimationFrame(() => {
      if (!this.isConnected) return;
      this.hitpointsBar.update(stats.hitpoints.current / stats.hitpoints.max);
      this.prayerBar.update(stats.prayer.current / stats.prayer.max);
      this.energyBar.update(stats.energy.current / stats.energy.max);
    });
  }

  updateText(stat, name) {
    const numbers = this.querySelector(`.player-stats__${name}-numbers`);
    if (!numbers) return;

    const currentStat = this[name];
    if (currentStat === undefined || currentStat.current !== stat.current || currentStat.max !== stat.max) {
      this[name] = stat;
      numbers.innerText = `${stat.current} / ${stat.max}`;
    }
  }

  handleActivePrayers(prayers) {
    this.activePrayers = prayers ?? [];
    window.requestAnimationFrame(() => {
      if (!this.isConnected) return;
      this.renderPrayerSlots();
    });
  }

  prayerIconHtml(prayerKey) {
    const iconUrl = Prayer.iconUrl(prayerKey);
    const name = Prayer.displayName(prayerKey);
    const image = iconUrl
      ? `<img src="${iconUrl}" alt="" />`
      : `<span class="player-stats__prayer-icon-fallback">${name.slice(0, 2).toUpperCase()}</span>`;
    return `<span class="player-stats__prayer-icon" title="${name}">${image}</span>`;
  }

  overflowChipHtml(hiddenPrayerKeys) {
    if (hiddenPrayerKeys.length === 0) return "";
    const names = hiddenPrayerKeys.map(Prayer.displayName).join(", ");
    return `<span class="player-stats__prayer-overflow" title="+${hiddenPrayerKeys.length} more: ${names}">+${hiddenPrayerKeys.length}</span>`;
  }

  renderPrayerSlots() {
    if (!this.prayerSlots) return;

    const sorted = Prayer.sortByPriority(this.activePrayers);
    if (sorted.length === 0) {
      this.prayerSlots.innerHTML = "";
      return;
    }

    this.prayerSlots.innerHTML = sorted.map((p) => this.prayerIconHtml(p)).join("");
    if (this.prayerSlots.scrollWidth <= this.prayerSlots.clientWidth) return;

    let visibleCount = sorted.length - 1;
    while (visibleCount > 0) {
      const visible = sorted.slice(0, visibleCount);
      const hidden = sorted.slice(visibleCount);
      this.prayerSlots.innerHTML = visible.map((p) => this.prayerIconHtml(p)).join("") + this.overflowChipHtml(hidden);
      if (this.prayerSlots.scrollWidth <= this.prayerSlots.clientWidth) return;
      visibleCount--;
    }
    this.prayerSlots.innerHTML = this.overflowChipHtml(sorted);
  }
}
customElements.define("player-stats", PlayerStats);
