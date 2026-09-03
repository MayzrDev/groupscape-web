import { BaseElement } from "../base-element/base-element";
import { slayerData } from "../data/slayer";

/**
 * Minibar tab (same swap-into-content pattern as `player-inventory`/`player-stats`/etc, see
 * `player-panel`'s `handleMiniBarClick`) showing a group member's current slayer task, streak
 * and points.
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
  }

  init(groupData) {
    this.member = groupData.members.get(this.playerName);
    this.render();
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

  amountDone() {
    const task = this.member.slayerTask;
    return Math.max(0, task.initialAmount - task.amountRemaining);
  }

  progressPercent() {
    const task = this.member.slayerTask;
    if (!task.initialAmount) return 0;
    return Math.max(0, Math.min(100, Math.round((this.amountDone() / task.initialAmount) * 100)));
  }

  renderNoTask() {
    return `<div class="slayer-panel__no-task">No task</div>`;
  }

  renderTask() {
    const task = this.member.slayerTask;
    // The plugin can occasionally fail to resolve a task's name off the game's own DB tables
    // (see SlayerTaskState#resolveTaskName) while amountRemaining/initialAmount still come
    // through fine - fall back to a placeholder rather than literally printing "null".
    const taskName = task.taskName ?? "Unknown task";
    const masterIcon = slayerData.masterIconUrl(task.masterName);
    const taskIcon = slayerData.taskIconUrl(task.taskName);

    return `
      ${
        task.masterName
          ? `
      <div class="slayer-panel__master">
        ${
          masterIcon ? `<img class="slayer-panel__master-portrait" src="${masterIcon}" alt="${task.masterName}" />` : ""
        }
        <span class="slayer-panel__master-name">${task.masterName}</span>
      </div>
      `
          : ""
      }

      <div class="slayer-panel__task">
        <img class="slayer-panel__task-icon" src="${taskIcon}" alt="${taskName}" />
        <div class="slayer-panel__task-body">
          <span class="slayer-panel__task-name">${taskName}</span>
          ${task.taskLocation ? `<span class="slayer-panel__task-location">${task.taskLocation}</span>` : ""}
          ${
            task.taskName
              ? `
          <a
            href="${slayerData.taskWikiUrl(task.taskName)}"
            target="_blank"
            rel="noopener"
            class="slayer-panel__wiki-link"
          >View slayer guide &#8599;</a>
          `
              : ""
          }
        </div>
      </div>

      <div class="slayer-panel__progress">
        <div class="slayer-panel__progress-track">
          <div class="slayer-panel__progress-fill" style="width: ${this.progressPercent()}%"></div>
        </div>
        <span class="slayer-panel__progress-frac">${this.amountDone()}/${task.initialAmount}</span>
      </div>
    `;
  }
}
customElements.define("slayer-panel", SlayerPanel);
