import { BaseElement } from "../base-element/base-element";
import { activityEventDescription, activityMetaLabel } from "../data/activity-event-copy";

const TOAST_DURATION_MS = 7000;
const LEAVE_ANIMATION_MS = 350;

const TOAST_ICONS = {
  kill: "⚔",
  death: "☠",
  quest: "✓",
  diary: "⛰",
  "combat-achievement": "✦",
  collection_log: "❖",
};

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
    this.subscribe("toast", this.handleToast.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  handleToast(toast) {
    const el = document.createElement("div");
    el.className = `toast-stack__toast toast-stack__toast--${toast.type}`;
    el.innerHTML = `
      <div class="toast-stack__icon">${TOAST_ICONS[toast.type] || "•"}</div>
      <div class="toast-stack__body">
        <div class="toast-stack__title">${this.titleHtml(toast)}</div>
        ${this.metaHtml(toast) ? `<div class="toast-stack__meta">${this.metaHtml(toast)}</div>` : ""}
      </div>
    `;
    this.list.appendChild(el);

    setTimeout(() => {
      el.classList.add("toast-stack__toast--leaving");
      el.addEventListener("animationend", () => el.remove());
      // Fallback in case prefers-reduced-motion skips the leave animation entirely.
      setTimeout(() => el.remove(), LEAVE_ANIMATION_MS);
    }, TOAST_DURATION_MS);
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
