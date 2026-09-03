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

  points() {
    return this.member?.slayerTask?.points ?? 0;
  }

  // `null`/`undefined` means that bucket has never been observed for this member yet (as
  // opposed to a real streak of 0) - rendered as "-" rather than 0, see renderStreakCell.
  streakNormal() {
    return this.member?.slayerTask?.streakNormal ?? null;
  }

  streakMortimer() {
    return this.member?.slayerTask?.streakMortimer ?? null;
  }

  streakWildy() {
    return this.member?.slayerTask?.streakWildy ?? null;
  }

  // Which bucket the member's *current* task belongs to, for highlighting the matching cell -
  // `null` while there's no active task, since masterName (and therefore the bucket) is only
  // known while one is assigned.
  activeStreakBucket() {
    if (!this.hasTask()) return null;
    const masterName = (this.member?.slayerTask?.masterName ?? "").trim().toLowerCase();
    if (masterName === "krystilia") return "wildy";
    if (masterName === "mortimer") return "mortimer";
    return "normal";
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

  renderStreakCell(bucket, label, value) {
    const isActive = this.activeStreakBucket() === bucket;
    return `
      <div class="slayer-panel__streak-cell${isActive ? " slayer-panel__streak-cell--active" : ""}">
        <span class="slayer-panel__stat-label">${label}</span>
        <span class="slayer-panel__stat-value">${
          value ?? '<span class="slayer-panel__stat-value--empty">&mdash;</span>'
        }</span>
      </div>
    `;
  }

  renderMaster() {
    const task = this.member.slayerTask;
    if (!task.masterName) return "";

    const masterIcon = slayerData.masterIconUrl(task.masterName);
    return `
      <a
        class="slayer-panel__master-banner"
        href="${slayerData.masterWikiUrl(task.masterName)}"
        target="_blank"
        rel="noopener"
      >
        ${
          masterIcon
            ? `<img class="slayer-panel__master-chathead" src="${masterIcon}" alt="${task.masterName}" />`
            : `<span class="slayer-panel__master-chathead slayer-panel__master-chathead--fallback"></span>`
        }
        <span class="slayer-panel__master-meta">
          <span class="slayer-panel__master-label">Slayer master</span>
          <span class="slayer-panel__master-name">${task.masterName}</span>
        </span>
      </a>
    `;
  }

  renderTask() {
    const task = this.member.slayerTask;
    // The plugin can occasionally fail to resolve a task's name off the game's own DB tables
    // (see SlayerTaskState#resolveTaskName) while amountRemaining/initialAmount still come
    // through fine - fall back to a placeholder rather than literally printing "null".
    const taskName = task.taskName ?? "Unknown task";
    const taskIcon = slayerData.taskIconUrl(task.taskName);

    return `
      ${this.renderMaster()}

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
