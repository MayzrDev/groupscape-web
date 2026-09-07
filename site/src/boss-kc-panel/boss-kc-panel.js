import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { hiscoreBossIconUrl } from "../data/boss-icons";
import { activityIconUrl, activityWikiUrl } from "../data/activity-icons";

/**
 * Minibar tab (same swap-into-content pattern as `slayer-panel`/`player-skills`/etc, see
 * `player-panel`'s `handleMiniBarClick`) showing a group member's REAL OSRS hiscores boss kill
 * counts, clue scroll counts, and minigame/activity scores - fetched live from Jagex's official
 * hiscores every time this tab is opened. Deliberately unrelated to GroupScape's own
 * self-tracked kill-count system (`leaderboard.rs`'s `BossKc` metric) - this only ever shows the
 * account's real lifetime hiscores totals.
 */
export class BossKcPanel extends BaseElement {
  constructor() {
    super();
    this.state = "loading";
    this.data = null;
    this.errorKind = null;
    this.errorMessage = null;
  }

  html() {
    return `{{boss-kc-panel.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.playerName = this.getAttribute("player-name");
    this.render();
    this.eventListener(this, "click", this.handleClick.bind(this));
    this.load();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  handleClick(event) {
    if (event.target.closest(".boss-kc-panel__retry")) {
      this.load();
    }
  }

  async load() {
    this.state = "loading";
    this.render();

    const result = await api.getBossKc(this.playerName);
    // The panel may have been torn down (tab switched away) while the fetch was in flight -
    // don't touch a detached element's DOM.
    if (!this.isConnected) return;

    if (!result.ok) {
      this.state = "error";
      this.errorKind = result.kind;
      this.errorMessage = result.message;
      this.render();
      return;
    }

    this.state = "loaded";
    this.data = result.data;
    this.render();
  }

  renderSkeletonRow() {
    return `
      <div class="boss-kc-panel__row">
        <div class="boss-kc-panel__cell boss-kc-panel__cell--skeleton"></div>
        <div class="boss-kc-panel__cell boss-kc-panel__cell--skeleton"></div>
        <div class="boss-kc-panel__cell boss-kc-panel__cell--skeleton"></div>
      </div>
    `;
  }

  renderLoading() {
    return `
      <div class="boss-kc-panel__loading">
        <span class="boss-kc-panel__spinner"></span>
        <span class="boss-kc-panel__loading-text">Fetching live hiscores&hellip;</span>
      </div>
      <div class="boss-kc-panel__section-label">Bosses</div>
      <div class="boss-kc-panel__grid">
        ${this.renderSkeletonRow()}
        ${this.renderSkeletonRow()}
        ${this.renderSkeletonRow()}
      </div>
      <div class="boss-kc-panel__section-label">Clues &amp; Minigames</div>
      <div class="boss-kc-panel__grid">
        ${this.renderSkeletonRow()}
        ${this.renderSkeletonRow()}
      </div>
    `;
  }

  renderError() {
    const isNotFound = this.errorKind === "not_found";
    const headline = isNotFound
      ? "Hiscores profile may be private or the account may not exist"
      : "OSRS hiscores are temporarily unavailable";
    const sub = isNotFound
      ? "GroupScape can't read this account's live hiscores right now."
      : "Jagex's hiscores may be down or rate-limiting requests - try again shortly.";
    return `
      <div class="boss-kc-panel__error">
        <span class="boss-kc-panel__error-headline">${headline}</span>
        <span class="boss-kc-panel__error-sub">${sub}</span>
        <button type="button" class="boss-kc-panel__retry">Retry</button>
      </div>
    `;
  }

  renderCell(activity) {
    const tracked = activity.kc >= 0;
    const iconUrl = activity.category === "boss" ? hiscoreBossIconUrl(activity.key) : activityIconUrl(activity.key);
    const value = tracked ? activity.kc.toLocaleString() : "&mdash;&mdash;";
    const tooltip = `${activity.name} - ${tracked ? activity.kc.toLocaleString() : "Untracked"}`;
    const wikiUrl = activityWikiUrl(activity);
    const tag = wikiUrl ? "a" : "div";
    const attrs = wikiUrl ? `href="${wikiUrl}" target="_blank" rel="noopener"` : "";
    return `
      <${tag} class="boss-kc-panel__cell${
      tracked ? "" : " boss-kc-panel__cell--untracked"
    }" title="${tooltip}" ${attrs}>
        <img class="boss-kc-panel__icon" src="${iconUrl}" alt="${activity.name}" />
        <span class="boss-kc-panel__value">${value}</span>
      </${tag}>
    `;
  }

  renderGrid(activities) {
    const rows = [];
    for (let i = 0; i < activities.length; i += 3) {
      rows.push(activities.slice(i, i + 3));
    }
    return `
      <div class="boss-kc-panel__grid">
        ${rows
          .map(
            (row) => `
          <div class="boss-kc-panel__row">
            ${row.map((activity) => this.renderCell(activity)).join("")}
          </div>
        `
          )
          .join("")}
      </div>
    `;
  }

  renderLoaded() {
    const activities = this.data?.activities ?? [];
    const bosses = activities.filter((a) => a.category === "boss");
    const cluesAndMinigames = activities.filter((a) => a.category === "clue" || a.category === "minigame");

    return `
      <div class="boss-kc-panel__section-label">Bosses</div>
      ${this.renderGrid(bosses)}
      <div class="boss-kc-panel__section-label">Clues &amp; Minigames</div>
      ${this.renderGrid(cluesAndMinigames)}
    `;
  }

  renderBody() {
    if (this.state === "loading") return this.renderLoading();
    if (this.state === "error") return this.renderError();
    return this.renderLoaded();
  }
}
customElements.define("boss-kc-panel", BossKcPanel);
