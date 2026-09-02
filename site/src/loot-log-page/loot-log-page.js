import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { utility } from "../utility";
import { Item } from "../data/item";
import { timeBounds, formatDuration, formatTimeRange } from "../data/time-range";

const PAGE_LIMIT = 25;
const REFRESH_INTERVAL_MS = 15000;
const SEARCH_DEBOUNCE_MS = 300;
// Same reasoning as activity-feed-page.js's AUTO_LOAD_BURST_LIMIT - a search can scan page after
// page of raw history without a single match (server-side scan cap notwithstanding), so bound how
// many pages the sentinel auto-fetches before requiring a manual click, rather than trying to
// detect "made no progress" (a near-miss search can also add a trickle of real matches per page).
const AUTO_LOAD_BURST_LIMIT = 4;
// Consecutive same-member/source/type events with a gap under this are one farming session -
// see the locked Loot Log design doc's "45-minute rolling window" rule.
const SESSION_MERGE_WINDOW_MS = 45 * 60 * 1000;
const MAX_RESOLVED_ITEM_IDS = 200;
const PLACEHOLDER_EXAMPLES = ["zulrah", ">1m", "whip", "732", "master clue", "<100k"];
const PLACEHOLDER_INTERVAL_MS = 2500;
const PLACEHOLDER_FADE_MS = 400;

export class LootLogPage extends BaseElement {
  constructor() {
    super();
    this.loaded = [];
    this.nextBefore = undefined;
    this.exhausted = false;
    this.loadingMore = false;
    this.autoLoadStreak = 0;
    this.autoLoadPaused = false;
    this.searchText = "";
    this.itemIds = [];
    this.summary = { total_value: 0, event_count: 0 };
    // key (member+source+type+clueTier, see `entryKey`) -> { key, events, element } for the
    // farming-session entry currently shown for that key, so an event within the 45-minute
    // window extends it instead of adding a new entry.
    this.entryGroups = new Map();
  }

  html() {
    return `{{loot-log-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.summaryContainer = this.querySelector(".loot-log-page__summary");
    this.searchInput = this.querySelector(".loot-log-page__search");
    this.placeholderLabel = this.querySelector(".loot-log-page__search-placeholder");
    this.list = this.querySelector(".loot-log-page__list");
    this.sentinel = this.querySelector(".loot-log-page__sentinel");
    this.empty = this.querySelector(".loot-log-page__empty");
    this.loadMoreButton = this.querySelector(".loot-log-page__load-more");
    this.scrollContainer = this.closest(".authed-section__main-content") || document.documentElement;

    this.eventListener(this.loadMoreButton, "click", () => this.loadMore({ manual: true }));
    this.eventListener(this.searchInput, "input", () => {
      this.stopPlaceholderCycle();
      window.clearTimeout(this.searchDebounceTimer);
      this.searchDebounceTimer = window.setTimeout(() => this.applySearch(), SEARCH_DEBOUNCE_MS);
    });
    this.eventListener(this.searchInput, "focus", () => this.stopPlaceholderCycle());
    this.eventListener(this.searchInput, "blur", () => {
      if (!this.searchInput.value) this.startPlaceholderCycle();
    });

    if (!this.searchInput.value) this.startPlaceholderCycle();

    this.intersectionObserver = new IntersectionObserver(this.handleSentinelIntersect.bind(this), {
      root: this.scrollContainer,
      rootMargin: "120px",
    });
    this.intersectionObserver.observe(this.sentinel);

    this.resetAndLoad();
    this.refreshInterval = utility.callOnInterval(this.poll.bind(this), REFRESH_INTERVAL_MS, false);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    if (this.refreshInterval) window.clearInterval(this.refreshInterval);
    if (this.intersectionObserver) this.intersectionObserver.disconnect();
    this.stopPlaceholderCycle();
    window.clearTimeout(this.searchDebounceTimer);
  }

  // IntersectionObserver only calls back on a threshold crossing, not continuously while
  // intersecting - a farming session collapses into one <loot-log-group> card, so loading another
  // page can leave the sentinel exactly where it was and the observer never fires again.
  // Re-observing after each load forces a fresh callback, but relying on the observer's internal
  // notification timing to drive every step of a loop proved flaky, so instead measure the
  // sentinel's actual position directly and keep loading for as long as it's really still in view.
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

  async autoLoadWhileVisible() {
    if (this.autoLoading) return;
    this.autoLoading = true;
    try {
      while (!this.autoLoadPaused && !this.exhausted && this.isSentinelInView()) {
        await this.loadMore();
      }
    } finally {
      this.autoLoading = false;
    }
  }

  startPlaceholderCycle() {
    if (!this.placeholderLabel || this.placeholderInterval) return;
    this.placeholderIndex = 0;
    this.placeholderLabel.textContent = PLACEHOLDER_EXAMPLES[0];
    this.placeholderLabel.classList.remove("loot-log-page__search-placeholder--hidden");
    this.placeholderInterval = window.setInterval(() => {
      this.placeholderLabel.classList.add("loot-log-page__search-placeholder--hidden");
      window.setTimeout(() => {
        this.placeholderIndex = (this.placeholderIndex + 1) % PLACEHOLDER_EXAMPLES.length;
        this.placeholderLabel.textContent = PLACEHOLDER_EXAMPLES[this.placeholderIndex];
        this.placeholderLabel.classList.remove("loot-log-page__search-placeholder--hidden");
      }, PLACEHOLDER_FADE_MS);
    }, PLACEHOLDER_INTERVAL_MS);
  }

  stopPlaceholderCycle() {
    if (this.placeholderInterval) {
      window.clearInterval(this.placeholderInterval);
      this.placeholderInterval = null;
    }
    if (this.placeholderLabel) this.placeholderLabel.classList.add("loot-log-page__search-placeholder--hidden");
  }

  // Heuristic client-side item-name resolution: the server has no full item-name table (only
  // curated drop_rates entries), so it can't search on item name itself - the client resolves
  // matching item ids from the full catalog (Item.itemDetails) and sends them as `item_ids` for
  // the server to test against each event's loot. Capped so a broad query can't blow up the URL.
  resolveItemIds(query) {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length || !Item.itemDetails) return [];
    const ids = [];
    for (const [id, details] of Object.entries(Item.itemDetails)) {
      const name = (details.name || "").toLowerCase();
      if (words.some((word) => name.includes(word))) {
        ids.push(Number(id));
        if (ids.length >= MAX_RESOLVED_ITEM_IDS) break;
      }
    }
    return ids;
  }

  applySearch() {
    const raw = this.searchInput.value.trim();
    this.searchText = raw;
    this.itemIds = this.resolveItemIds(raw);
    this.resetAndLoad();
  }

  entryKey(event) {
    return `${event.member_name}|${event.source_name}|${event.source_type}|${event.clue_tier || ""}`;
  }

  buildGroupData(entry) {
    const newest = entry.events[0];
    return {
      memberName: newest.member_name,
      sourceName: newest.source_name,
      sourceType: newest.source_type,
      clueTier: newest.clue_tier,
      events: entry.events,
    };
  }

  // Appended when paging older history in (see `loadMore`) - events arrive newest-first within a
  // page, so each new event is older than whatever's already tracked for its key.
  appendEvent(event) {
    const key = this.entryKey(event);
    const entry = this.entryGroups.get(key);
    const eventTime = new Date(event.occurred_at).getTime();
    if (entry) {
      const oldestTime = new Date(entry.events[entry.events.length - 1].occurred_at).getTime();
      if (oldestTime - eventTime <= SESSION_MERGE_WINDOW_MS) {
        entry.events.push(event);
        entry.element.group = this.buildGroupData(entry);
        entry.element.update();
        return;
      }
    }
    const newEntry = { key, events: [event] };
    newEntry.element = document.createElement("loot-log-group");
    newEntry.element.group = this.buildGroupData(newEntry);
    this.list.appendChild(newEntry.element);
    this.entryGroups.set(key, newEntry);
  }

  // Prepended when polling for fresher events (see `poll`) - events arrive oldest-of-batch first
  // (caller reverses the incoming batch), so each new event is newer than whatever's tracked.
  prependEvent(event) {
    const key = this.entryKey(event);
    const entry = this.entryGroups.get(key);
    const eventTime = new Date(event.occurred_at).getTime();
    if (entry) {
      const newestTime = new Date(entry.events[0].occurred_at).getTime();
      if (eventTime - newestTime <= SESSION_MERGE_WINDOW_MS) {
        entry.events.unshift(event);
        entry.element.group = this.buildGroupData(entry);
        entry.element.update();
        this.list.insertBefore(entry.element, this.list.firstChild);
        return;
      }
    }
    const newEntry = { key, events: [event] };
    newEntry.element = document.createElement("loot-log-group");
    newEntry.element.group = this.buildGroupData(newEntry);
    this.list.insertBefore(newEntry.element, this.list.firstChild);
    this.entryGroups.set(key, newEntry);
  }

  async resetAndLoad() {
    this.loaded = [];
    this.nextBefore = undefined;
    this.exhausted = false;
    this.autoLoadStreak = 0;
    this.autoLoadPaused = false;
    this.entryGroups = new Map();
    this.list.innerHTML = "";
    this.empty.classList.remove("loot-log-page__empty--visible");
    this.sentinel.classList.remove("loot-log-page__sentinel--paused");
    await Promise.all([this.loadMore(), this.loadSummary()]);
  }

  async loadSummary() {
    this.summary = await api.getLootLogSummary({ search: this.searchText, itemIds: this.itemIds });
    this.renderSummary();
  }

  async loadMore({ manual = false } = {}) {
    if (this.loadingMore || this.exhausted) return;
    if (this.autoLoadPaused && !manual) return;
    if (manual) {
      this.autoLoadPaused = false;
      this.autoLoadStreak = 0;
      this.sentinel.classList.remove("loot-log-page__sentinel--paused");
    } else {
      this.autoLoadStreak++;
    }
    this.loadingMore = true;
    this.sentinel.classList.add("loot-log-page__sentinel--visible");

    const page = await api.getLootLog({
      before: this.nextBefore,
      limit: PAGE_LIMIT,
      search: this.searchText,
      itemIds: this.itemIds,
    });

    for (const event of page.events) {
      this.loaded.push(event);
      this.appendEvent(event);
    }
    this.nextBefore = page.next_before || undefined;
    this.exhausted = !!page.scan_exhausted;
    this.autoLoadPaused = !this.exhausted && this.autoLoadStreak >= AUTO_LOAD_BURST_LIMIT;
    this.sentinel.classList.toggle("loot-log-page__sentinel--visible", !this.exhausted);
    this.sentinel.classList.toggle("loot-log-page__sentinel--paused", this.autoLoadPaused);
    this.empty.classList.toggle("loot-log-page__empty--visible", this.loaded.length === 0 && this.exhausted);
    this.renderSummary();
    this.loadingMore = false;
  }

  // Mirrors activity-feed-page.js's `poll` - fetches the newest page and prepends whatever is
  // newer than what's already loaded, preserving scroll position the same way.
  async poll() {
    if (this.loadingMore || !this.loaded.length) return;

    const newestOccurredAt = new Date(this.loaded[0].occurred_at).getTime();
    const page = await api.getLootLog({ search: this.searchText, itemIds: this.itemIds, limit: PAGE_LIMIT });
    const incoming = page.events.filter((event) => new Date(event.occurred_at).getTime() > newestOccurredAt);
    if (!incoming.length) return;

    const scrollTopBefore = this.scrollContainer.scrollTop;
    const heightBefore = this.list.scrollHeight;

    for (const event of [...incoming].reverse()) {
      this.loaded.unshift(event);
      this.prependEvent(event);
    }

    if (scrollTopBefore > 0) {
      const delta = this.list.scrollHeight - heightBefore;
      this.scrollContainer.scrollTop = scrollTopBefore + delta;
    }

    await this.loadSummary();
  }

  renderSummary() {
    if (!this.summaryContainer) return;
    if (this.loaded.length === 0 && this.summary.event_count === 0) {
      this.summaryContainer.innerHTML = "";
      return;
    }
    this.summaryContainer.innerHTML = `
      <div class="loot-log-page__summary-stats">
        <div class="loot-log-page__summary-card">
          <div class="loot-log-page__summary-n">${this.summary.total_value.toLocaleString()}</div>
          <div class="loot-log-page__summary-l">Total Value</div>
        </div>
        <div class="loot-log-page__summary-card">
          <div class="loot-log-page__summary-n">${this.summary.event_count.toLocaleString()}</div>
          <div class="loot-log-page__summary-l">Events</div>
        </div>
        <div class="loot-log-page__summary-card loot-log-page__summary-card--session">
          <div class="loot-log-page__summary-n">${this.sessionDuration()}</div>
          <div class="loot-log-page__summary-l">${this.sessionRange()}</div>
        </div>
      </div>
    `;
  }

  // First loaded event -> last loaded event, not a detected play-session boundary - just the
  // span of what's currently paged in (loading more history extends it).
  sessionDuration() {
    if (!this.loaded.length) return "0m";
    const { first, last } = timeBounds(this.loaded);
    return formatDuration(first, last);
  }

  sessionRange() {
    if (!this.loaded.length) return "";
    const { first, last } = timeBounds(this.loaded);
    return formatTimeRange(first, last);
  }
}

customElements.define("loot-log-page", LootLogPage);
