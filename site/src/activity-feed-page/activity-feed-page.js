import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { pubsub } from "../data/pubsub";
import { utility } from "../utility";

const EVENT_TYPES = [
  [null, "All"],
  ["kill", "Kills"],
  ["death", "Deaths"],
  ["quest", "Quests"],
  ["diary", "Diaries"],
  ["combat_task", "Combat tasks"],
  ["collection_log", "Collection log"],
  ["clue", "Clues"],
  ["raid", "Raids"],
];

const PAGE_LIMIT = 25;
const COUNT_LIMIT = 200;
const REFRESH_INTERVAL_MS = 15000;

export class ActivityFeedPage extends BaseElement {
  constructor() {
    super();
    this.members = [];
    this.selectedMember = null;
    this.selectedType = null;
    this.loaded = [];
    this.typeCountEvents = [];
    this.memberCountEvents = [];
    this.loadingMore = false;
    this.exhausted = false;
  }

  html() {
    return `{{activity-feed-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.rail = this.querySelector(".activity-feed-page__rail");
    this.memberFilters = this.querySelector(".activity-feed-page__member-filters");
    this.list = this.querySelector(".activity-feed-page__list");
    this.sentinel = this.querySelector(".activity-feed-page__sentinel");
    this.empty = this.querySelector(".activity-feed-page__empty");
    this.scrollContainer = this.closest(".authed-section__main-content") || document.documentElement;

    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));
    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }

    this.renderRail();
    this.resetAndLoad();

    this.intersectionObserver = new IntersectionObserver(this.handleSentinelIntersect.bind(this), {
      root: this.scrollContainer,
      rootMargin: "120px",
    });
    this.intersectionObserver.observe(this.sentinel);

    this.refreshInterval = utility.callOnInterval(this.poll.bind(this), REFRESH_INTERVAL_MS, false);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    if (this.refreshInterval) window.clearInterval(this.refreshInterval);
    if (this.intersectionObserver) this.intersectionObserver.disconnect();
  }

  handleUpdatedMembers(members) {
    this.members = members.filter((member) => member.name !== "@SHARED");
    this.renderMemberFilters();
  }

  handleSentinelIntersect(entries) {
    if (entries.some((entry) => entry.isIntersecting)) this.loadMore();
  }

  // Best-effort counts, not true totals — there's no cheap way to get an exact total from a
  // cursor-paginated feed without a dedicated count query. Counted from a pool fetched ignoring
  // the axis being counted (but still respecting the *other* axis' filter), so picking a type
  // doesn't zero out every other type's count and vice versa.
  loadedCountForMember(memberName) {
    return this.memberCountEvents.filter((event) => !memberName || event.member_name === memberName).length;
  }

  loadedCountForType(type) {
    return this.typeCountEvents.filter((event) => !type || event.event_type === type).length;
  }

  renderRail() {
    if (!this.rail) return;
    this.rail.innerHTML = "";
    for (const [type, label] of EVENT_TYPES) {
      this.rail.appendChild(this.createTypeRailButton(type, label, this.loadedCountForType(type)));
    }
  }

  createTypeRailButton(type, label, count) {
    const button = document.createElement("button");
    button.className = "activity-feed-page__rail-btn";
    if (type) button.classList.add(`activity-feed-page__rail-btn--${type}`);
    button.classList.toggle("activity-feed-page__rail-btn--active", this.selectedType === type);
    button.innerHTML = `
      <span class="activity-feed-page__rail-label">${label}</span>
      <span class="activity-feed-page__rail-count">${count}</span>
    `;
    this.eventListener(button, "click", () => {
      if (this.selectedType === type) return;
      this.selectedType = type;
      this.renderRail();
      this.resetAndLoad();
    });
    return button;
  }

  renderMemberFilters() {
    if (!this.memberFilters) return;
    this.memberFilters.innerHTML = "";

    const allChip = this.createMemberChip(null, "All members", this.loadedCountForMember(null));
    this.memberFilters.appendChild(allChip);

    for (const member of this.members) {
      this.memberFilters.appendChild(
        this.createMemberChip(member.name, member.name, this.loadedCountForMember(member.name))
      );
    }
  }

  createMemberChip(memberName, label, count) {
    const chip = document.createElement("button");
    chip.className = "activity-feed-page__member-chip";
    chip.classList.toggle("activity-feed-page__member-chip--active", this.selectedMember === memberName);
    chip.innerHTML = `
      ${memberName ? `<player-icon player-name="${memberName}"></player-icon>` : ""}
      <span class="activity-feed-page__member-chip-label">${label}</span>
      <span class="activity-feed-page__member-chip-count">${count}</span>
    `;
    this.eventListener(chip, "click", () => {
      if (this.selectedMember === memberName) return;
      this.selectedMember = memberName;
      this.renderMemberFilters();
      this.resetAndLoad();
    });
    return chip;
  }

  createRow(event) {
    const row = document.createElement("activity-feed-event");
    row.event = event;
    return row;
  }

  async resetAndLoad() {
    this.loaded = [];
    this.exhausted = false;
    this.list.innerHTML = "";
    this.empty.classList.remove("activity-feed-page__empty--visible");
    await Promise.all([this.loadMore(), this.loadCounts()]);
  }

  async loadCounts() {
    const [typeCountEvents, memberCountEvents] = await Promise.all([
      api.getActivityEvents({ memberName: this.selectedMember, limit: COUNT_LIMIT }),
      api.getActivityEvents({ eventType: this.selectedType, limit: COUNT_LIMIT }),
    ]);
    this.typeCountEvents = typeCountEvents;
    this.memberCountEvents = memberCountEvents;
    this.renderRail();
    this.renderMemberFilters();
  }

  async loadMore() {
    if (this.loadingMore || this.exhausted) return;
    this.loadingMore = true;
    this.sentinel.classList.add("activity-feed-page__sentinel--visible");

    const before = this.loaded.length ? this.loaded[this.loaded.length - 1].occurred_at : undefined;
    const page = await api.getActivityEvents({
      memberName: this.selectedMember,
      eventType: this.selectedType,
      before,
      limit: PAGE_LIMIT,
    });

    for (const event of page) {
      this.loaded.push(event);
      this.list.appendChild(this.createRow(event));
    }
    this.exhausted = page.length < PAGE_LIMIT;
    this.sentinel.classList.toggle("activity-feed-page__sentinel--visible", !this.exhausted);
    this.empty.classList.toggle("activity-feed-page__empty--visible", this.loaded.length === 0);
    this.renderRail();
    this.renderMemberFilters();
    this.loadingMore = false;
  }

  // Fetches the newest page and prepends whatever is newer than what's already loaded, leaving
  // already-loaded older pages untouched. Preserves the reader's scroll position by measuring how
  // much content height the prepend adds and offsetting scrollTop by the same amount, so a poll
  // firing while the user is scrolled down never yanks the viewport back to the top.
  async poll() {
    if (this.loadingMore || !this.loaded.length) return;

    const newestOccurredAt = new Date(this.loaded[0].occurred_at).getTime();
    const page = await api.getActivityEvents({
      memberName: this.selectedMember,
      eventType: this.selectedType,
      limit: PAGE_LIMIT,
    });
    const incoming = page.filter((event) => new Date(event.occurred_at).getTime() > newestOccurredAt);
    if (!incoming.length) return;

    const scrollTopBefore = this.scrollContainer.scrollTop;
    const heightBefore = this.list.scrollHeight;

    const fragment = document.createDocumentFragment();
    for (const event of incoming) {
      this.loaded.unshift(event);
      const row = this.createRow(event);
      row.classList.add("activity-feed-event--enter");
      fragment.appendChild(row);
    }
    this.list.insertBefore(fragment, this.list.firstChild);

    if (scrollTopBefore > 0) {
      const delta = this.list.scrollHeight - heightBefore;
      this.scrollContainer.scrollTop = scrollTopBefore + delta;
    }

    await this.loadCounts();
  }
}

customElements.define("activity-feed-page", ActivityFeedPage);
