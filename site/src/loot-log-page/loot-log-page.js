import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { utility } from "../utility";
import { Item } from "../data/item";
import { timeBounds, formatDuration, formatTimeRange } from "../data/time-range";

// Server clamps this to 100 (see get_loot_log's `query.limit.clamp(1, 100)`) - use the max so a
// typical group's history resolves in as few round trips as possible, since every trip risks
// landing mid-session and needing the GROUP_CLOSE_FETCH_CAP chase below.
const PAGE_LIMIT = 100;
const REFRESH_INTERVAL_MS = 15000;
const SEARCH_DEBOUNCE_MS = 300;
// Same reasoning as activity-feed-page.js's AUTO_LOAD_BURST_LIMIT - a search can scan page after
// page of raw history without a single match (server-side scan cap notwithstanding), so bound how
// many pages the sentinel auto-fetches before requiring a manual click, rather than trying to
// detect "made no progress" (a near-miss search can also add a trickle of real matches per page).
// Kept fairly high, unlike activity-feed's, since a real page here (see PAGE_LIMIT) already does
// much more work per request - a burst of these covers a lot more history before pausing.
const AUTO_LOAD_BURST_LIMIT = 10;
// Unconditional backstop on the sentinel auto-load loop, independent of autoLoadPaused/exhausted -
// guarantees the loop can never run away even if that state is ever wrong.
const AUTO_LOAD_HARD_CAP = 20;
// Consecutive same-member/source/type events with a gap under this are one farming session -
// see the locked Loot Log design doc's "45-minute rolling window" rule.
const SESSION_MERGE_WINDOW_MS = 45 * 60 * 1000;
// A session's group card must never grow after it's been shown to the user - so the oldest
// (trailing) card produced by a loadMore() page can't be considered "loaded" until we know its
// 45-minute window is actually closed, which requires peeking at the next page too. Bounds how
// many extra pages a single loadMore() call may fetch chasing that closure - same reasoning as
// AUTO_LOAD_BURST_LIMIT: an uninterrupted same-boss farming session can span many pages, so this
// has to give up and leave the trailing card open (it'll keep resolving on subsequent loadMore()
// calls) rather than risk the runaway-fetch tab freeze this file has been bitten by before.
const GROUP_CLOSE_FETCH_CAP = 5;
const MAX_RESOLVED_ITEM_IDS = 200;
const PLACEHOLDER_EXAMPLES = ["zulrah", ">1m", "whip", "732", "master clue", "<100k"];
const PLACEHOLDER_INTERVAL_MS = 2500;
const PLACEHOLDER_FADE_MS = 400;

export class LootLogPage extends BaseElement {
  constructor() {
    super();
    this.disposed = false;
    this.loaded = [];
    this.nextBefore = undefined;
    this.exhausted = false;
    this.loadingMore = false;
    this.autoLoadStreak = 0;
    this.autoLoadPaused = false;
    // Total events added since the streak last reset (manual click or resetAndLoad) - lets
    // loadMore() tell "still making progress, just needs another click" apart from "repeatedly
    // scanned and found nothing", so the sentinel can hide itself in the latter case instead of
    // dangling a "Load more" that's unlikely to ever surface anything (see `autoLoadGaveUp`).
    this.autoLoadStreakEventsAdded = 0;
    this.autoLoadGaveUp = false;
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
    // app-route reuses the same page instance across navigations (only constructs it once - see
    // app-route.js's `enable()`), so disconnectedCallback's `disposed = true` would otherwise stick
    // forever after the first time this page is navigated away from, silently no-op'ing every
    // loadMore/loadSummary/poll call on every future visit.
    this.disposed = false;
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

    // On a hard refresh landing directly on this page, this component's connectedCallback can run
    // before app-initializer's api.enable() finishes setting the group credentials - loading then
    // instead of waiting for it fired requests as `/api/group/undefined/...`, which 401 and get
    // misread as "no more history", permanently marking the page exhausted with nothing loaded.
    // Wait for the first successful group-data fetch (the same signal app-initializer itself waits
    // on) before loading if the group isn't ready yet.
    if (api.groupName) {
      this.resetAndLoad();
    } else {
      this.subscribeOnce("get-group-data", () => this.resetAndLoad());
    }
    this.refreshInterval = utility.callOnInterval(this.poll.bind(this), REFRESH_INTERVAL_MS, false);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    // Stops any in-flight loadMore/poll/auto-load-loop from touching state or firing further
    // requests once this component is torn down - e.g. the group session getting invalidated
    // mid-request (see api.js's disable()-on-401) used to leave a stray autoLoadWhileVisible loop
    // running against a detached component with no group credentials, hammering the API with
    // `/api/group/undefined/...` requests that looked like a stuck "Load more".
    this.disposed = true;
    if (this.refreshInterval) window.clearInterval(this.refreshInterval);
    if (this.intersectionObserver) this.intersectionObserver.disconnect();
    this.stopPlaceholderCycle();
    window.clearTimeout(this.searchDebounceTimer);
  }

  // IntersectionObserver only calls back on a threshold crossing, not continuously while
  // intersecting - a farming session collapses into one <loot-log-group> card, so loading another
  // page can leave the sentinel exactly where it was and the observer never fires again.
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
        this.disposed ||
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
      if (this.disposed) return;
      requestAnimationFrame(step);
    };
    step();
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
  // page, so each new event is older than whatever's already tracked for its key. A `frozen`
  // entry (see `loadMore`'s GROUP_CLOSE_FETCH_CAP handling) has already been shown to the user as
  // a finished card and must never grow again, even if a later event would otherwise fall inside
  // its window - falls through to starting a fresh entry for that key instead.
  appendEvent(event) {
    const key = this.entryKey(event);
    const entry = this.entryGroups.get(key);
    const eventTime = new Date(event.occurred_at).getTime();
    if (entry && !entry.frozen) {
      const oldestTime = new Date(entry.events[entry.events.length - 1].occurred_at).getTime();
      if (oldestTime - eventTime <= SESSION_MERGE_WINDOW_MS) {
        entry.events.push(event);
        entry.element.group = this.buildGroupData(entry);
        entry.element.update();
        return;
      }
    }
    const newEntry = { key, events: [event], frozen: false };
    newEntry.element = document.createElement("loot-log-group");
    newEntry.element.group = this.buildGroupData(newEntry);
    this.list.appendChild(newEntry.element);
    this.entryGroups.set(key, newEntry);
  }

  // Prepended when polling for fresher events (see `poll`) - events arrive oldest-of-batch first
  // (caller reverses the incoming batch), so each new event is newer than whatever's tracked.
  // Same `frozen` guard as `appendEvent`, though in practice a poll's fresh events landing inside
  // a frozen (old, already-closed-out) entry's window would be a rare coincidence.
  prependEvent(event) {
    const key = this.entryKey(event);
    const entry = this.entryGroups.get(key);
    const eventTime = new Date(event.occurred_at).getTime();
    if (entry && !entry.frozen) {
      const newestTime = new Date(entry.events[0].occurred_at).getTime();
      if (eventTime - newestTime <= SESSION_MERGE_WINDOW_MS) {
        entry.events.unshift(event);
        entry.element.group = this.buildGroupData(entry);
        entry.element.update();
        this.list.insertBefore(entry.element, this.list.firstChild);
        return;
      }
    }
    const newEntry = { key, events: [event], frozen: false };
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
    this.autoLoadStreakEventsAdded = 0;
    this.autoLoadGaveUp = false;
    this.entryGroups = new Map();
    this.list.innerHTML = "";
    this.empty.classList.remove("loot-log-page__empty--visible");
    this.sentinel.classList.remove("loot-log-page__sentinel--paused");
    await Promise.all([this.loadMore(), this.loadSummary()]);
  }

  async loadSummary() {
    if (!api.groupName) return;
    const summary = await api.getLootLogSummary({ search: this.searchText, itemIds: this.itemIds });
    if (this.disposed) return;
    this.summary = summary;
    this.renderSummary();
  }

  async loadMore({ manual = false } = {}) {
    if (this.loadingMore || this.exhausted) return;
    if (this.autoLoadPaused && !manual) return;
    // No group session to load against - e.g. it was invalidated mid-view (see disconnectedCallback).
    // Bail out before touching any state so a stray call can't leave the sentinel/button stuck.
    if (!api.groupName) return;
    if (manual) {
      this.autoLoadPaused = false;
      this.autoLoadStreak = 0;
      this.autoLoadStreakEventsAdded = 0;
      this.autoLoadGaveUp = false;
      this.sentinel.classList.remove("loot-log-page__sentinel--paused");
    } else {
      this.autoLoadStreak++;
    }
    this.loadingMore = true;
    this.sentinel.classList.add("loot-log-page__sentinel--visible");
    this.sentinel.classList.add("loot-log-page__sentinel--loading");

    // Keep fetching until the trailing group is provably closed (see `GROUP_CLOSE_FETCH_CAP`):
    // a page's last event always leaves its group's fate unknown - only the next page's leading
    // event, once fetched, can tell us whether that group grows further. Events only get older
    // from here, so as soon as one page's first event fails to extend the current trailing group,
    // that group can never grow again and it's provably closed (no freeze needed - the window
    // check alone blocks any future merge into it). Each new page's own tail then becomes the
    // next trailing group to verify, chained until it's closed, exhausted, or the fetch budget
    // runs out.
    let trailingKey = null;
    let addedThisCall = 0;
    let fetches = 0;
    for (; fetches < GROUP_CLOSE_FETCH_CAP; fetches++) {
      const page = await api.getLootLog({
        before: this.nextBefore,
        limit: PAGE_LIMIT,
        search: this.searchText,
        itemIds: this.itemIds,
      });
      if (this.disposed) {
        this.loadingMore = false;
        return;
      }

      if (trailingKey && page.events.length) {
        const first = page.events[0];
        const entry = this.entryGroups.get(trailingKey);
        const extendsTrailing =
          entry &&
          !entry.frozen &&
          this.entryKey(first) === trailingKey &&
          new Date(entry.events[entry.events.length - 1].occurred_at).getTime() -
            new Date(first.occurred_at).getTime() <=
            SESSION_MERGE_WINDOW_MS;
        if (!extendsTrailing) trailingKey = null;
      }

      for (const event of page.events) {
        this.loaded.push(event);
        this.appendEvent(event);
      }
      addedThisCall += page.events.length;
      this.nextBefore = page.next_before || undefined;
      this.exhausted = !!page.scan_exhausted;
      trailingKey = this.exhausted || !page.events.length ? null : this.entryKey(page.events[page.events.length - 1]);

      if (!trailingKey) break;
    }

    // Fetch budget ran out chasing a group's closure without resolving it - freeze that card so
    // it can't keep silently growing on a future loadMore() call (see `appendEvent`'s `frozen`
    // guard). It stays as whatever it loaded here; a real session that long may render as more
    // than one card.
    if (trailingKey) {
      const openEntry = this.entryGroups.get(trailingKey);
      if (openEntry) openEntry.frozen = true;
    }

    this.loadingMore = false;
    this.sentinel.classList.remove("loot-log-page__sentinel--loading");
    if (this.disposed) return;

    this.autoLoadStreakEventsAdded += addedThisCall;
    this.autoLoadPaused = !this.exhausted && this.autoLoadStreak >= AUTO_LOAD_BURST_LIMIT;
    // The server's raw-row scan is capped per request (see get_loot_log's LOOT_LOG_SCAN_CAP), so
    // a long run of loot-less kills near the end of a group's history can repeatedly report "not
    // exhausted" without ever actually surfacing anything new. Once a full auto-load burst has
    // come back completely empty, treat it the same as exhausted for display purposes - a "Load
    // more" that's already proven fruitless a few times in a row isn't worth dangling in front of
    // the user, even though it's not a mathematically certain end of history.
    this.autoLoadGaveUp = this.autoLoadPaused && this.autoLoadStreakEventsAdded === 0;
    this.sentinel.classList.toggle("loot-log-page__sentinel--visible", !this.exhausted && !this.autoLoadGaveUp);
    this.sentinel.classList.toggle("loot-log-page__sentinel--paused", this.autoLoadPaused && !this.autoLoadGaveUp);
    this.empty.classList.toggle("loot-log-page__empty--visible", this.loaded.length === 0 && this.exhausted);
    this.renderSummary();
  }

  // Mirrors activity-feed-page.js's `poll` - fetches the newest page and prepends whatever is
  // newer than what's already loaded, preserving scroll position the same way.
  async poll() {
    if (this.loadingMore || !this.loaded.length || !api.groupName) return;

    const newestOccurredAt = new Date(this.loaded[0].occurred_at).getTime();
    const page = await api.getLootLog({ search: this.searchText, itemIds: this.itemIds, limit: PAGE_LIMIT });
    if (this.disposed) return;
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
