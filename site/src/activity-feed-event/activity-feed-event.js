import { BaseElement } from "../base-element/base-element";
import { activityBadgeLabel, activityDisplayType, activityEventDescription } from "../data/activity-event-copy";
import { Item } from "../data/item";

const RELATIVE_UNITS = [
  ["y", 31536000],
  ["mo", 2592000],
  ["d", 86400],
  ["h", 3600],
  ["m", 60],
];

function formatRelativeTime(date) {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return "just now";

  for (const [suffix, unitSeconds] of RELATIVE_UNITS) {
    const value = Math.floor(seconds / unitSeconds);
    if (value >= 1) return `${value}${suffix} ago`;
  }
  return "just now";
}

export class ActivityFeedEvent extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{activity-feed-event.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    if (this.displayType === "clue" && this.event.payload?.clueTier) {
      this.style.setProperty("--clue-tier-color", `var(--clue-${this.event.payload.clueTier})`);
    }
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  get occurredAt() {
    return new Date(this.event.occurred_at);
  }

  get relativeTime() {
    return formatRelativeTime(this.occurredAt);
  }

  get loot() {
    // A merged raid completion has no top-level `payload.loot` - each reporting member's share
    // lives under its own `participants[].loot` entry instead (see `RaidCompletionPayload`).
    const loot = ["cox", "tob", "toa"].includes(this.displayType)
      ? (this.event.payload?.participants || []).flatMap((p) => p.loot || [])
      : this.event.payload?.loot || [];
    // Same "unrecognized item" filter loot-log-group.js applies before rendering an item-box -
    // an item id with no local catalog entry (not yet added, or a bad drop report) would otherwise
    // throw inside item-box's Item.imageUrl() and take the whole row's render() down with it.
    return loot.filter((entry) => Item.exists(entry.item_id));
  }

  get displayType() {
    return activityDisplayType(this.event);
  }

  get badgeLabel() {
    return activityBadgeLabel(this.event);
  }

  descriptionHtml() {
    return activityEventDescription(this.event, {
      member: (name) => `<span class="activity-feed-event__member">${name}</span>`,
      subject: (text, variant, wikiUrl, iconSrc, secondaryIconSrc) => {
        const cls = `activity-feed-event__subject${variant === "death" ? " activity-feed-event__subject--death" : ""}${
          variant === "clue" ? " activity-feed-event__subject--clue" : ""
        }`;
        const icons = [iconSrc, secondaryIconSrc]
          .filter(Boolean)
          .map((src) => `<img class="activity-feed-event__subject-icon" src="${src}" alt="" />`)
          .join("");
        return wikiUrl
          ? `<a href="${wikiUrl}" target="_blank" rel="noopener" class="${cls}">${text}${icons}</a>`
          : `<span class="${cls}">${text}${icons}</span>`;
      },
    });
  }
}

customElements.define("activity-feed-event", ActivityFeedEvent);
