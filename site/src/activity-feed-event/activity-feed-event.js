import { BaseElement } from "../base-element/base-element";

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

  get isKill() {
    return this.event.event_type === "kill";
  }

  get isDeath() {
    return this.event.event_type === "death";
  }

  get loot() {
    return this.event.payload?.loot || [];
  }

  descriptionHtml() {
    const member = `<span class="activity-feed-event__member">${this.event.member_name}</span>`;
    if (this.isKill) {
      const npc = `<span class="activity-feed-event__subject">${this.event.payload.npc_name}</span>`;
      return `${member} killed ${npc}${this.loot.length === 0 ? " — no loot" : ""}`;
    }
    if (this.isDeath) {
      const killerName = this.event.payload?.killer_name;
      return killerName
        ? `${member} died to <span class="activity-feed-event__subject activity-feed-event__subject--death">${killerName}</span>`
        : `${member} died`;
    }
    return `${member} — ${this.event.event_type}`;
  }
}

customElements.define("activity-feed-event", ActivityFeedEvent);
