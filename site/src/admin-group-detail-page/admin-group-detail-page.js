import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { requireAdmin } from "../data/admin-guard";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";

export class AdminGroupDetailPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-group-detail-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.groupId = new URLSearchParams(window.location.search).get("groupId");
    this.render();
    this.init();
  }

  async init() {
    if (!(await requireAdmin())) return;
    if (!this.groupId) {
      this.querySelector(".admin-group-detail__error").textContent = "No group selected.";
      return;
    }
    await this.fetchGroup();
  }

  async fetchGroup() {
    const error = this.querySelector(".admin-group-detail__error");
    try {
      const response = await adminApi.getGroup(this.groupId);
      if (response.status === 404) {
        error.textContent = "Group not found.";
        return;
      }
      if (!response.ok) {
        error.textContent = "Failed to load group.";
        return;
      }
      error.textContent = "";
      this.group = await response.json();
      this.renderGroup();
    } catch (e) {
      error.textContent = `Failed to load group: ${e}`;
    }
  }

  renderGroup() {
    const group = this.group;
    this.querySelector(".admin-group-detail__title").textContent = group.group_name;

    const banner = this.querySelector(".admin-group-detail__banner");
    banner.className = `admin-group-detail__banner admin-status-banner admin-status-banner--${group.status}`;
    const statusLabel = group.status.charAt(0).toUpperCase() + group.status.slice(1);
    const reasonText = group.reason ? ` &middot; Reason: ${group.reason}` : "";
    this.querySelector(".admin-group-detail__status-text").innerHTML = `<strong>${statusLabel}</strong>${reasonText}`;

    const actions = this.querySelector(".admin-group-detail__actions");
    let actionsHtml = "";
    if (group.status === "active") {
      actionsHtml += `<button class="admin-btn js-suspend">Suspend</button>`;
      actionsHtml += `<button class="admin-btn admin-btn--danger js-ban">Ban</button>`;
    } else if (group.status === "suspended") {
      actionsHtml += `<button class="admin-btn js-unban">Reactivate</button>`;
      actionsHtml += `<button class="admin-btn admin-btn--danger js-ban">Ban</button>`;
    } else {
      actionsHtml += `<button class="admin-btn js-unban">Reactivate</button>`;
      actionsHtml += `<button class="admin-btn admin-btn--danger js-delete">Delete group&hellip;</button>`;
    }
    actions.innerHTML = actionsHtml;

    const suspendButton = actions.querySelector(".js-suspend");
    const banButton = actions.querySelector(".js-ban");
    const unbanButton = actions.querySelector(".js-unban");
    const deleteButton = actions.querySelector(".js-delete");
    if (suspendButton) this.eventListener(suspendButton, "click", this.suspend.bind(this));
    if (banButton) this.eventListener(banButton, "click", this.ban.bind(this));
    if (unbanButton) this.eventListener(unbanButton, "click", this.unban.bind(this));
    if (deleteButton) this.eventListener(deleteButton, "click", this.deleteGroup.bind(this));

    const roster = this.querySelector(".admin-group-detail__roster");
    roster.innerHTML =
      group.members.length > 0
        ? group.members.map((name) => `<div class="admin-roster__member">${name}</div>`).join("")
        : `<div class="admin-roster__empty">No members.</div>`;
    this.querySelector(".admin-group-detail__roster-count").textContent = group.members.length;
  }

  async suspend() {
    await this.runAction(async () => {
      const response = await adminApi.suspendGroup(this.groupId, null);
      if (!response.ok) throw new Error("Failed to suspend group");
      await this.fetchGroup();
    });
  }

  async unban() {
    await this.runAction(async () => {
      const response = await adminApi.unbanGroup(this.groupId);
      if (!response.ok) throw new Error("Failed to reactivate group");
      await this.fetchGroup();
    });
  }

  ban() {
    confirmDialogManager.confirm({
      headline: `Ban ${this.group.group_name}?`,
      body: "The group will be locked out until an admin reactivates it.",
      yesCallback: () =>
        this.runAction(async () => {
          const response = await adminApi.banGroup(this.groupId, null);
          if (!response.ok) throw new Error("Failed to ban group");
          await this.fetchGroup();
        }),
      noCallback: () => {},
    });
  }

  deleteGroup() {
    confirmDialogManager.confirm({
      headline: `Delete ${this.group.group_name}?`,
      body: "All members and player data for this group will be permanently deleted. This cannot be undone.",
      yesCallback: () =>
        this.runAction(async () => {
          const response = await adminApi.deleteGroup(this.groupId);
          if (!response.ok) throw new Error("Failed to delete group");
          window.history.pushState("", "", "/admin/groups");
        }),
      noCallback: () => {},
    });
  }

  async runAction(action) {
    const error = this.querySelector(".admin-group-detail__error");
    try {
      loadingScreenManager.showLoadingScreen();
      error.textContent = "";
      await action();
    } catch (e) {
      error.textContent = e.message ?? String(e);
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }
}

customElements.define("admin-group-detail-page", AdminGroupDetailPage);
