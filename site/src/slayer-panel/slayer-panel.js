import { BaseElement } from "../base-element/base-element";
import { slayerData } from "../data/slayer";

/**
 * Small anchored popover (not the full-screen dialog pattern `combat-achievements` uses) showing
 * a group member's current slayer task, streak and points. Opened from `player-panel`'s minibar,
 * which sets `.anchor` to the button it should hang below before appending this element.
 */
export class SlayerPanel extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{slayer-panel.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.playerName = this.getAttribute("player-name");
    this.subscribeOnce("get-group-data", this.init.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    document.removeEventListener("mousedown", this.handleOutsideClickBound);
    window.removeEventListener("resize", this.positionBound);
    window.removeEventListener("scroll", this.positionBound, true);
  }

  init(groupData) {
    this.member = groupData.members.get(this.playerName);
    this.render();

    this.eventListener(this.querySelector(".slayer-panel__close"), "click", this.close.bind(this));

    this.handleOutsideClickBound = this.handleOutsideClick.bind(this);
    this.positionBound = this.position.bind(this);
    // mousedown (not click) so the button's own click - which opened this panel - doesn't
    // immediately reach here and close it again as an "outside" click.
    document.addEventListener("mousedown", this.handleOutsideClickBound);
    window.addEventListener("resize", this.positionBound);
    window.addEventListener("scroll", this.positionBound, true);

    this.position();
  }

  close() {
    this.remove();
  }

  handleOutsideClick(event) {
    if (this.contains(event.target) || this.anchor?.contains(event.target)) return;
    this.close();
  }

  position() {
    if (!this.anchor || !this.isConnected) return;
    const popover = this.querySelector(".slayer-panel__popover");
    if (!popover) return;

    const anchorRect = this.anchor.getBoundingClientRect();
    const popoverRect = popover.getBoundingClientRect();
    const margin = 6;

    let left = anchorRect.left;
    if (left + popoverRect.width > window.innerWidth - margin) {
      left = window.innerWidth - popoverRect.width - margin;
    }
    left = Math.max(margin, left);

    let top = anchorRect.bottom + margin;
    if (top + popoverRect.height > window.innerHeight - margin) {
      top = anchorRect.top - popoverRect.height - margin;
    }
    top = Math.max(margin, top);

    popover.style.left = `${left}px`;
    popover.style.top = `${top}px`;
  }

  hasTask() {
    return !!this.member?.slayerTask?.hasTask;
  }

  streak() {
    return this.member?.slayerTask?.streak ?? 0;
  }

  points() {
    return this.member?.slayerTask?.points ?? 0;
  }

  progressPercent() {
    const task = this.member.slayerTask;
    if (!task.initialAmount) return 0;
    const done = task.initialAmount - task.amountRemaining;
    return Math.max(0, Math.min(100, Math.round((done / task.initialAmount) * 100)));
  }

  renderNoTask() {
    return `<div class="slayer-panel__no-task">No task</div>`;
  }

  renderTask() {
    const task = this.member.slayerTask;
    const masterIcon = slayerData.masterIconUrl(task.masterName);
    const taskIcon = slayerData.taskIconUrl(task.taskName);

    return `
      <div class="slayer-panel__master">
        ${
          masterIcon ? `<img class="slayer-panel__master-portrait" src="${masterIcon}" alt="${task.masterName}" />` : ""
        }
        <span class="slayer-panel__master-name">${task.masterName}</span>
      </div>

      <div class="slayer-panel__task">
        <img class="slayer-panel__task-icon" src="${taskIcon}" alt="${task.taskName}" />
        <div class="slayer-panel__task-body">
          <span class="slayer-panel__task-name">${task.taskName}</span>
          ${task.taskLocation ? `<span class="slayer-panel__task-location">${task.taskLocation}</span>` : ""}
          <a
            href="${slayerData.taskWikiUrl(task.taskName)}"
            target="_blank"
            rel="noopener"
            class="slayer-panel__wiki-link"
          >View slayer guide &#8599;</a>
        </div>
      </div>

      <div class="slayer-panel__progress">
        <div class="slayer-panel__progress-track">
          <div class="slayer-panel__progress-fill" style="width: ${this.progressPercent()}%"></div>
        </div>
        <span class="slayer-panel__progress-frac">${task.amountRemaining}/${task.initialAmount}</span>
      </div>
    `;
  }
}
customElements.define("slayer-panel", SlayerPanel);
