import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { pubsub } from "../data/pubsub";
import { utility } from "../utility";

const EVENT_TYPES = [
  [null, "All"],
  ["kill", "Kills"],
  ["death", "Deaths"],
];

const FETCH_LIMIT = 100;
const REFRESH_INTERVAL_MS = 15000;

export class ActivityFeedPage extends BaseElement {
  constructor() {
    super();
    this.allEvents = [];
    this.members = [];
    this.selectedMember = null;
    this.selectedType = null;
  }

  html() {
    return `{{activity-feed-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.rail = this.querySelector(".activity-feed-page__rail");
    this.typeFilters = this.querySelector(".activity-feed-page__type-filters");
    this.list = this.querySelector(".activity-feed-page__list");
    this.empty = this.querySelector(".activity-feed-page__empty");

    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));
    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }

    this.renderTypeFilters();
    this.fetchEvents();
    this.refreshInterval = utility.callOnInterval(this.fetchEvents.bind(this), REFRESH_INTERVAL_MS, false);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    if (this.refreshInterval) window.clearInterval(this.refreshInterval);
  }

  handleUpdatedMembers(members) {
    this.members = members.filter((member) => member.name !== "@SHARED");
    this.renderRail();
  }

  async fetchEvents() {
    this.allEvents = await api.getActivityEvents({ limit: FETCH_LIMIT });
    this.renderRail();
    this.renderList();
  }

  eventCountFor(memberName) {
    return this.allEvents.filter((event) => event.member_name === memberName).length;
  }

  renderRail() {
    if (!this.rail) return;
    this.rail.innerHTML = "";

    const allButton = this.createRailButton(null, "All members", this.allEvents.length);
    this.rail.appendChild(allButton);

    for (const member of this.members) {
      this.rail.appendChild(this.createRailButton(member.name, member.name, this.eventCountFor(member.name)));
    }
  }

  createRailButton(memberName, label, count) {
    const button = document.createElement("button");
    button.className = "activity-feed-page__rail-btn";
    button.classList.toggle("activity-feed-page__rail-btn--active", this.selectedMember === memberName);
    button.innerHTML = `
      ${memberName ? `<player-icon player-name="${memberName}"></player-icon>` : ""}
      <span class="activity-feed-page__rail-label">${label}</span>
      <span class="activity-feed-page__rail-count">${count}</span>
    `;
    this.eventListener(button, "click", () => {
      this.selectedMember = memberName;
      this.renderRail();
      this.renderList();
    });
    return button;
  }

  renderTypeFilters() {
    this.typeFilters.innerHTML = "";
    for (const [type, label] of EVENT_TYPES) {
      const chip = document.createElement("button");
      chip.className = "activity-feed-page__type-chip";
      chip.classList.toggle("activity-feed-page__type-chip--active", this.selectedType === type);
      chip.textContent = label;
      this.eventListener(chip, "click", () => {
        this.selectedType = type;
        this.renderTypeFilters();
        this.renderList();
      });
      this.typeFilters.appendChild(chip);
    }
  }

  filteredEvents() {
    return this.allEvents.filter((event) => {
      if (this.selectedMember && event.member_name !== this.selectedMember) return false;
      if (this.selectedType && event.event_type !== this.selectedType) return false;
      return true;
    });
  }

  renderList() {
    const events = this.filteredEvents();
    this.list.innerHTML = "";
    this.empty.classList.toggle("activity-feed-page__empty--visible", events.length === 0);

    for (const event of events) {
      const row = document.createElement("activity-feed-event");
      row.event = event;
      this.list.appendChild(row);
    }
  }
}

customElements.define("activity-feed-page", ActivityFeedPage);
