import { BaseElement } from "../base-element/base-element";
import { pubsub } from "../data/pubsub";
import { utility } from "../utility";

const COUNTDOWN_REFRESH_MS = 30000;

const CATEGORY_LABEL = { herb: "Herb Patches", tree: "Tree Patches", birdhouse: "Bird Houses" };
// Time Tracking has no notion of separate icons per category - both farming patch types share
// the Farming skill icon, bird houses use the Hunter skill icon (they're placed via Hunter).
const CATEGORY_ICON = { herb: "/ui/217-0.png", tree: "/ui/217-0.png", birdhouse: "/ui/220-0.png" };
const CATEGORY_ORDER = ["herb", "tree", "birdhouse"];

function formatCountdown(readyAt) {
  const diffMs = readyAt * 1000 - Date.now();
  if (diffMs <= 0) return "Ready";
  const totalMinutes = Math.ceil(diffMs / 60000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function slotState(entry) {
  if (entry.unconfirmed) return "unknown";
  if (entry.status === "harvestable") return "ready";
  if (entry.status === "growing" || entry.status === "seeded") return "growing";
  if (entry.status === "diseased" || entry.status === "dead") return "diseased";
  return "empty";
}

function slotOverlayText(entry) {
  if (entry.unconfirmed) return "?";
  switch (entry.status) {
    case "harvestable":
      return "Ready";
    case "growing":
    case "seeded":
      return entry.readyAt ? formatCountdown(entry.readyAt) : "";
    case "diseased":
      return "Sick";
    case "dead":
      return "Dead";
    case "built":
      return "No seed";
    default:
      return "";
  }
}

function statusText(entry) {
  if (entry.unconfirmed) return "Unknown — check patch";
  switch (entry.status) {
    case "harvestable":
      return "Ready to harvest";
    case "growing":
      return entry.readyAt ? `Growing — ready in ${formatCountdown(entry.readyAt)}` : "Growing";
    case "seeded":
      return entry.readyAt ? `Ready in ${formatCountdown(entry.readyAt)}` : "Seeded";
    case "diseased":
      return "Diseased";
    case "dead":
      return "Dead — needs clearing";
    case "built":
      return "Built, not seeded";
    case "empty":
      return "Empty";
    default:
      return "Unknown";
  }
}

function escapeAttribute(value) {
  return value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}

function groupByCategory(entries) {
  const grouped = { herb: [], tree: [], birdhouse: [] };
  for (const entry of entries) {
    if (grouped[entry.category]) grouped[entry.category].push(entry);
  }
  return grouped;
}

export class TimersPage extends BaseElement {
  constructor() {
    super();
    this.members = [];
  }

  html() {
    return `{{timers-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.membersContainer = this.querySelector(".timers-page__members");
    this.emptyMessage = this.querySelector(".timers-page__empty");

    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));
    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }

    // farming_timers rides the same 1s member poll as everything else, so no separate fetch is
    // needed here - this interval only re-renders the already-known countdowns as time passes.
    this.countdownInterval = utility.callOnInterval(() => this.renderMembers(), COUNTDOWN_REFRESH_MS, false);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    if (this.countdownInterval) window.clearInterval(this.countdownInterval);
  }

  handleUpdatedMembers(members) {
    this.members = members.filter((member) => member.name !== "@SHARED");
    this.renderMembers();
  }

  renderMembers() {
    if (!this.membersContainer) return;

    this.emptyMessage.classList.toggle("timers-page__empty--visible", this.members.length === 0);

    this.membersContainer.innerHTML = this.members.map((member) => this.renderMember(member)).join("");
  }

  renderMember(member) {
    const head = `
      <div class="timers-page__member-head">
        <player-icon player-name="${member.name}"></player-icon>
        <span class="timers-page__member-name">${member.name}</span>
      </div>
    `;

    if (!member.farmingTimers || member.farmingTimers.length === 0) {
      return `
        <div class="timers-page__member">
          ${head}
          <div class="timers-page__member-na">Time Tracking not detected for this member.</div>
        </div>
      `;
    }

    const grouped = groupByCategory(member.farmingTimers);
    const categories = CATEGORY_ORDER.map((category) => {
      const entries = grouped[category];
      if (entries.length === 0) return "";
      return `
        <div class="timers-page__category">
          <div class="timers-page__category-label">${CATEGORY_LABEL[category]}</div>
          <div class="timers-page__slot-grid">
            ${entries.map((entry) => this.renderSlot(entry)).join("")}
          </div>
        </div>
      `;
    }).join("");

    return `
      <div class="timers-page__member">
        ${head}
        <div class="timers-page__categories">${categories}</div>
      </div>
    `;
  }

  renderSlot(entry) {
    const state = slotState(entry);
    const tooltip = `${entry.label} — ${statusText(entry)}`;
    return `
      <div class="timers-page__slot timers-page__slot--${state}" title="${escapeAttribute(tooltip)}">
        <img class="timers-page__slot-img" loading="lazy" src="${CATEGORY_ICON[entry.category]}" alt="" />
        <span class="timers-page__slot-dot"></span>
        <span class="timers-page__slot-overlay">${slotOverlayText(entry)}</span>
      </div>
    `;
  }
}

customElements.define("timers-page", TimersPage);
