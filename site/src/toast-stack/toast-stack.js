import { BaseElement } from "../base-element/base-element";

const TOAST_DURATION_MS = 7000;
const LEAVE_ANIMATION_MS = 350;

const TOAST_ICONS = {
  kill: "⚔",
  death: "☠",
  quest: "✓",
  "combat-achievement": "✦",
  "level-up": "↑",
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

  // Phrasing convention for group-milestone copy across this app: "<member> <verb> <subject>"
  // (e.g. "Bandos killed Vorkath", "Zezima finished Monkey Madness II", "Woox reached level 90
  // Slayer") - plain past-tense sentences naming the actual boss/quest/tier/skill, not a generic
  // "New activity" label. Discord webhook payloads (#35) and low-HP/wilderness alerts (#37-39)
  // should read the same way.
  titleHtml(toast) {
    switch (toast.type) {
      case "kill": {
        const npc = toast.event.payload?.npc_name || "an NPC";
        const noLoot = !toast.event.payload?.loot || toast.event.payload.loot.length === 0;
        return `${this.subject(toast.event.member_name)} killed ${npc}${noLoot ? " — no loot" : ""}`;
      }
      case "death": {
        const killer = toast.event.payload?.killer_name;
        return killer
          ? `${this.subject(toast.event.member_name)} died to ${killer}`
          : `${this.subject(toast.event.member_name)} died`;
      }
      case "quest":
        return `${this.subject(toast.memberName)} finished ${toast.questName}`;
      case "combat-achievement":
        return `${this.subject(toast.memberName)} completed ${toast.tierLabel} tier`;
      case "level-up":
        return `${this.subject(toast.memberName)} reached level ${toast.level} ${toast.skillName}`;
      default:
        return this.subject(toast.memberName || toast.event?.member_name || "");
    }
  }

  metaHtml(toast) {
    if (toast.type === "combat-achievement") return "Combat Achievements";
    return "";
  }
}

customElements.define("toast-stack", ToastStack);
