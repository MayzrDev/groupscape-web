import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { pubsub } from "../data/pubsub";
import { utility } from "../utility";
import { activityDisplayType, killGroupKey, KILL_MERGE_WINDOW_MS } from "../data/activity-event-copy";

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
// A page of raw kill events can collapse to just one new visible row when the rest merge into it
// (see `mergeOrCreateRow`) - a long, roughly-continuous same-boss farming history does this for
// page after page (each ~hour-long chunk of kills becomes one more row), so watching for a
// *zero*-growth streak never catches it: there's always some trickle of progress, just far too
// little to justify silently fetching hundreds of pages back-to-back. Cap how many pages the
// sentinel is allowed to auto-fetch in a row regardless of how much they added, and require a
// manual click to keep going past that - bounds the worst case to a fixed, fast amount of work.
const AUTO_LOAD_BURST_LIMIT = 4;
// Unconditional backstop on the sentinel auto-load loop, independent of autoLoadPaused/exhausted -
// guarantees the loop can never run away even if that state is ever wrong.
const AUTO_LOAD_HARD_CAP = 20;

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
    this.autoLoadStreak = 0;
    this.autoLoadPaused = false;
    // key (member+boss, see `killGroupKey`) -> { event, row } for the kill currently shown as the
    // aggregated row for that key, so a repeat kill can be folded in rather than adding a row.
    this.feedGroups = new Map();
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
    this.loadMoreButton = this.querySelector(".activity-feed-page__load-more");
    this.eventListener(this.loadMoreButton, "click", () => this.loadMore({ manual: true }));
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

  // IntersectionObserver only calls back on a threshold crossing, not continuously while
  // intersecting - if a page load doesn't move the sentinel, the observer never fires again.
  // A tight `while` loop chaining `await`s on quick-resolving promises never actually yields to
  // the browser's render/input loop (recursive microtasks starve it even though each step is
  // "async"), which is how a past version of this froze the tab. Pace each step through
  // requestAnimationFrame instead, and cap the run length unconditionally as a backstop in case
  // autoLoadPaused/exhausted are ever wrong.
  handleSentinelIntersect(entries) {
    if (this.autoLoadPaused) return;
    if (entries.some((entry) => entry.isIntersecting)) this.autoLoadWhileVisible();
  }

  isSentinelInView() {
    const sentinelRect = this.sentinel.getBoundingClientRect();
    const containerRect =
      this.scrollContainer === document.documentElement
        ? { top: 0, bottom: window.innerHeight }
        : this.scrollContainer.getBoundingClientRect();
    return sentinelRect.top < containerRect.bottom + 120 && sentinelRect.bottom > containerRect.top - 120;
  }

  autoLoadWhileVisible() {
    if (this.autoLoading) return;
    this.autoLoading = true;
    this.autoLoadRunLength = 0;
    const step = async () => {
      if (
        this.autoLoadPaused ||
        this.exhausted ||
        this.autoLoadRunLength >= AUTO_LOAD_HARD_CAP ||
        !this.isSentinelInView()
      ) {
        this.autoLoading = false;
        return;
      }
      this.autoLoadRunLength++;
      await this.loadMore();
      requestAnimationFrame(step);
    };
    step();
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

  // Folds a repeat kill of the same boss by the same member into its existing aggregated row
  // (see `KILL_MERGE_WINDOW_MS`) instead of creating a new one. Returns the row to insert, or
  // null when the event was merged into an already-rendered row instead.
  mergeOrCreateRow(event, { prepend }) {
    if (activityDisplayType(event) === "kill") {
      const key = killGroupKey(event);
      const group = this.feedGroups.get(key);
      const eventTime = new Date(event.occurred_at).getTime();
      if (group && Math.abs(eventTime - new Date(group.event.occurred_at).getTime()) <= KILL_MERGE_WINDOW_MS) {
        group.event.aggregateCount = (group.event.aggregateCount || 1) + 1;
        group.event.payload = {
          ...group.event.payload,
          loot: [...(group.event.payload?.loot || []), ...(event.payload?.loot || [])],
        };
        if (eventTime > new Date(group.event.occurred_at).getTime()) group.event.occurred_at = event.occurred_at;
        group.row.event = group.event;
        group.row.render();
        // Only a fresh, more recent kill (poll) should bump the row back to the top - an older
        // kill filled in from loadMore already sits above the batch being appended below it.
        if (prepend) this.list.insertBefore(group.row, this.list.firstChild);
        return null;
      }
    }

    const row = this.createRow(event);
    if (activityDisplayType(event) === "kill") {
      this.feedGroups.set(killGroupKey(event), { event, row });
    }
    return row;
  }

  async resetAndLoad() {
    this.loaded = [];
    this.exhausted = false;
    this.autoLoadStreak = 0;
    this.autoLoadPaused = false;
    this.feedGroups = new Map();
    this.list.innerHTML = "";
    this.empty.classList.remove("activity-feed-page__empty--visible");
    this.sentinel.classList.remove("activity-feed-page__sentinel--paused");
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

  async loadMore({ manual = false } = {}) {
    if (this.loadingMore || this.exhausted) return;
    if (this.autoLoadPaused && !manual) return;
    if (manual) {
      this.autoLoadPaused = false;
      this.autoLoadStreak = 0;
      this.sentinel.classList.remove("activity-feed-page__sentinel--paused");
    } else {
      this.autoLoadStreak++;
    }
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
      const row = this.mergeOrCreateRow(event, { prepend: false });
      if (row) this.list.appendChild(row);
    }
    this.exhausted = page.length < PAGE_LIMIT;
    this.autoLoadPaused = !this.exhausted && this.autoLoadStreak >= AUTO_LOAD_BURST_LIMIT;
    this.sentinel.classList.toggle("activity-feed-page__sentinel--visible", !this.exhausted);
    this.sentinel.classList.toggle("activity-feed-page__sentinel--paused", this.autoLoadPaused);
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

    // Oldest-of-batch first so each insertion lands above the previous one, keeping merge
    // repositioning (see `mergeOrCreateRow`) in the right chronological order at the top.
    for (const event of [...incoming].reverse()) {
      this.loaded.unshift(event);
      const row = this.mergeOrCreateRow(event, { prepend: true });
      if (row) {
        row.classList.add("activity-feed-event--enter");
        this.list.insertBefore(row, this.list.firstChild);
      }
    }

    if (scrollTopBefore > 0) {
      const delta = this.list.scrollHeight - heightBefore;
      this.scrollContainer.scrollTop = scrollTopBefore + delta;
    }

    await this.loadCounts();
  }
}

customElements.define("activity-feed-page", ActivityFeedPage);
