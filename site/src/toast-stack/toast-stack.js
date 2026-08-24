import { BaseElement } from "../base-element/base-element";
import { activityEventDescription, activityMetaLabel, activityLinkFor } from "../data/activity-event-copy";

const LEAVE_ANIMATION_MS = 300;
const RELATIVE_TIME_TICK_MS = 1000;

const TOAST_ICONS = {
  kill: "⚔",
  death: "☠",
  quest: "✓",
  diary: "⛰",
  "combat-achievement": "✦",
  collection_log: "❖",
};

function relativeTime(fromDate) {
  const seconds = Math.floor((Date.now() - fromDate.getTime()) / 1000);
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}

// Toasts are meant to sit in the corner while the app runs unattended, so they stay on screen
// until the player dismisses them rather than auto-expiring after a few seconds.
export class ToastStack extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{toast-stack.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.list = this.querySelector(".toast-stack__list");
    this.clearButton = this.querySelector(".toast-stack__clear");
    this.eventListener(this.clearButton, "click", () => this.clearAll());
    this.subscribe("toast", this.handleToast.bind(this));
    this.tickInterval = window.setInterval(this.tickTimestamps.bind(this), RELATIVE_TIME_TICK_MS);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    window.clearInterval(this.tickInterval);
  }

  handleToast(toast) {
    const el = document.createElement("a");
    el.className = `toast-stack__toast toast-stack__toast--${toast.type}`;
    el.href = toast.event ? activityLinkFor(toast.event) : "/group/activity";
    el.dataset.occurredAt = toast.event?.occurred_at || new Date().toISOString();
    el.innerHTML = `
      <div class="toast-stack__icon">${TOAST_ICONS[toast.type] || "•"}</div>
      <div class="toast-stack__body">
        <div class="toast-stack__title">${this.titleHtml(toast)}</div>
        <div class="toast-stack__meta">
          ${
            this.metaHtml(toast)
              ? `<span>${this.metaHtml(toast)}</span><span class="toast-stack__dot">&middot;</span>`
              : ""
          }
          <span class="toast-stack__time"></span>
        </div>
      </div>
      <button class="toast-stack__close" type="button" aria-label="Dismiss">&times;</button>
    `;
    this.eventListener(
      el.querySelector(".toast-stack__close"),
      "click",
      (event) => {
        event.preventDefault();
        event.stopPropagation();
        this.dismiss(el);
      },
      { passive: false }
    );
    this.eventListener(
      el,
      "click",
      (event) => {
        event.preventDefault();
        window.history.pushState("", "", el.href);
        this.dismiss(el);
      },
      { passive: false }
    );
    this.updateTimestamp(el);
    this.list.appendChild(el);
    this.updateClearButton();
  }

  dismiss(el) {
    el.classList.add("toast-stack__toast--leaving");
    el.addEventListener("animationend", () => el.remove());
    // Fallback in case prefers-reduced-motion skips the leave animation entirely.
    setTimeout(() => {
      el.remove();
      this.updateClearButton();
    }, LEAVE_ANIMATION_MS);
  }

  clearAll() {
    this.list.querySelectorAll(".toast-stack__toast:not(.toast-stack__toast--leaving)").forEach((el) => {
      this.dismiss(el);
    });
    this.updateClearButton();
  }

  updateClearButton() {
    this.clearButton.hidden = !this.list.querySelector(".toast-stack__toast:not(.toast-stack__toast--leaving)");
  }

  tickTimestamps() {
    this.list.querySelectorAll(".toast-stack__toast").forEach((el) => this.updateTimestamp(el));
  }

  updateTimestamp(el) {
    const timeEl = el.querySelector(".toast-stack__time");
    if (timeEl) timeEl.textContent = relativeTime(new Date(el.dataset.occurredAt));
  }

  subject(name) {
    return `<span class="toast-stack__subject">${name}</span>`;
  }

  // Copy is shared with the activity feed (see `data/activity-event-copy.js`) - the toast just
  // highlights the member name instead of the subject.
  titleHtml(toast) {
    if (!toast.event) return this.subject(toast.memberName || "");
    return activityEventDescription(toast.event, { member: (name) => this.subject(name) });
  }

  metaHtml(toast) {
    if (!toast.event) return "";
    return activityMetaLabel(toast.event);
  }
}

customElements.define("toast-stack", ToastStack);
