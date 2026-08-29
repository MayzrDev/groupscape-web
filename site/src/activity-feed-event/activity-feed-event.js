import { BaseElement } from "../base-element/base-element";
import { activityBadgeLabel, activityEventDescription } from "../data/activity-event-copy";

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
    return this.event.payload?.loot || [];
  }

  get badgeLabel() {
    return activityBadgeLabel(this.event.event_type);
  }

  descriptionHtml() {
    return activityEventDescription(this.event, {
      member: (name) => `<span class="activity-feed-event__member">${name}</span>`,
      subject: (text, variant, wikiUrl, iconSrc) => {
        const cls = `activity-feed-event__subject${variant === "death" ? " activity-feed-event__subject--death" : ""}`;
        const icon = iconSrc ? `<img class="activity-feed-event__subject-icon" src="${iconSrc}" alt="" />` : "";
        return wikiUrl
          ? `<a href="${wikiUrl}" target="_blank" rel="noopener" class="${cls}">${icon}${text}</a>`
          : `<span class="${cls}">${icon}${text}</span>`;
      },
    });
  }
}

customElements.define("activity-feed-event", ActivityFeedEvent);
