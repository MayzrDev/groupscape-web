import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { requireAdmin } from "../data/admin-guard";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";

const PAGE_SIZE = 24;

export class AdminGroupsPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-groups-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.search = "";
    this.page = 1;
    this.groups = [];
    this.total = 0;
    this.render();

    this.searchInput = this.querySelector(".admin-groups__search");
    this.cardGrid = this.querySelector(".admin-groups__grid");
    this.pager = this.querySelector(".admin-groups__pager");
    this.statTotal = this.querySelector(".admin-groups__stat-total");
    this.statPageSuspended = this.querySelector(".admin-groups__stat-suspended");
    this.statPageBanned = this.querySelector(".admin-groups__stat-banned");
    this.error = this.querySelector(".admin-groups__error");

    this.eventListener(this.searchInput, "input", this.handleSearchInput.bind(this));
    this.eventListener(this.querySelector(".js-prev-page"), "click", this.prevPage.bind(this));
    this.eventListener(this.querySelector(".js-next-page"), "click", this.nextPage.bind(this));

    this.init();
  }

  async init() {
    if (!(await requireAdmin())) return;
    await this.fetchGroups();
  }

  handleSearchInput() {
    clearTimeout(this.searchDebounce);
    this.searchDebounce = setTimeout(() => {
      this.search = this.searchInput.value.trim();
      this.page = 1;
      this.fetchGroups();
    }, 300);
  }

  async fetchGroups() {
    try {
      const response = await adminApi.listGroups(this.search, this.page, PAGE_SIZE);
      if (!response.ok) {
        this.error.textContent = "Failed to load groups.";
        return;
      }
      const data = await response.json();
      this.groups = data.groups;
      this.total = data.total;
      this.error.textContent = "";
      this.renderGroups();
    } catch (error) {
      this.error.textContent = `Failed to load groups: ${error}`;
    }
  }

  renderGroups() {
    this.statTotal.textContent = this.total;
    this.statPageSuspended.textContent = this.groups.filter((g) => g.status === "suspended").length;
    this.statPageBanned.textContent = this.groups.filter((g) => g.status === "banned").length;

    this.cardGrid.innerHTML = this.groups.map((group) => this.cardHtml(group)).join("");
    for (const card of this.cardGrid.querySelectorAll(".admin-group-card")) {
      const groupId = card.dataset.groupId;
      const viewButton = card.querySelector(".js-view");
      const suspendButton = card.querySelector(".js-suspend");
      const banButton = card.querySelector(".js-ban");
      const unbanButton = card.querySelector(".js-unban");
      const deleteButton = card.querySelector(".js-delete");

      if (viewButton) this.eventListener(viewButton, "click", () => this.viewGroup(groupId));
      if (suspendButton) this.eventListener(suspendButton, "click", () => this.suspendGroup(groupId));
      if (banButton) this.eventListener(banButton, "click", () => this.banGroup(groupId));
      if (unbanButton) this.eventListener(unbanButton, "click", () => this.unbanGroup(groupId));
      if (deleteButton) this.eventListener(deleteButton, "click", () => this.deleteGroup(groupId));
    }

    const totalPages = Math.max(1, Math.ceil(this.total / PAGE_SIZE));
    this.pager.querySelector(".admin-groups__pager-label").textContent = `Page ${this.page} of ${totalPages}`;
    this.pager.querySelector(".js-prev-page").disabled = this.page <= 1;
    this.pager.querySelector(".js-next-page").disabled = this.page >= totalPages;
  }

  prevPage() {
    if (this.page <= 1) return;
    this.page -= 1;
    this.fetchGroups();
  }

  nextPage() {
    const totalPages = Math.max(1, Math.ceil(this.total / PAGE_SIZE));
    if (this.page >= totalPages) return;
    this.page += 1;
    this.fetchGroups();
  }

  cardHtml(group) {
    const badgeClass = group.status === "active" ? "good" : group.status === "suspended" ? "warn" : "bad";
    const statusLabel = group.status.charAt(0).toUpperCase() + group.status.slice(1);
    let actions = `<button class="admin-btn admin-btn--small js-view">View</button>`;
    if (group.status === "active") {
      actions += `<button class="admin-btn admin-btn--small js-suspend">Suspend</button>`;
      actions += `<button class="admin-btn admin-btn--small admin-btn--danger js-ban">Ban</button>`;
    } else if (group.status === "suspended") {
      actions += `<button class="admin-btn admin-btn--small js-unban">Reactivate</button>`;
      actions += `<button class="admin-btn admin-btn--small admin-btn--danger js-ban">Ban</button>`;
    } else {
      actions += `<button class="admin-btn admin-btn--small js-unban">Reactivate</button>`;
      actions += `<button class="admin-btn admin-btn--small admin-btn--danger js-delete">Delete</button>`;
    }

    return `
      <div class="admin-group-card" data-group-id="${group.group_id}">
        <div class="admin-group-card__top">
          <span class="admin-group-card__name">${group.group_name}</span>
          <span class="admin-badge admin-badge--${badgeClass}">${statusLabel}</span>
        </div>
        <span class="admin-group-card__meta">#${group.group_id} &middot; ${group.member_count} members</span>
        <div class="admin-group-card__actions">${actions}</div>
      </div>
    `;
  }

  viewGroup(groupId) {
    window.history.pushState("", "", `/admin/group-detail?groupId=${groupId}`);
  }

  async suspendGroup(groupId) {
    await this.runAction(async () => {
      const response = await adminApi.suspendGroup(groupId, null);
      if (!response.ok) throw new Error("Failed to suspend group");
      await this.fetchGroups();
    });
  }

  async unbanGroup(groupId) {
    await this.runAction(async () => {
      const response = await adminApi.unbanGroup(groupId);
      if (!response.ok) throw new Error("Failed to reactivate group");
      await this.fetchGroups();
    });
  }

  banGroup(groupId) {
    const group = this.groups.find((g) => String(g.group_id) === String(groupId));
    confirmDialogManager.confirm({
      headline: `Ban ${group?.group_name ?? "this group"}?`,
      body: "The group will be locked out until an admin reactivates it.",
      yesCallback: () =>
        this.runAction(async () => {
          const response = await adminApi.banGroup(groupId, null);
          if (!response.ok) throw new Error("Failed to ban group");
          await this.fetchGroups();
        }),
      noCallback: () => {},
    });
  }

  deleteGroup(groupId) {
    const group = this.groups.find((g) => String(g.group_id) === String(groupId));
    confirmDialogManager.confirm({
      headline: `Delete ${group?.group_name ?? "this group"}?`,
      body: "All members and player data for this group will be permanently deleted. This cannot be undone.",
      yesCallback: () =>
        this.runAction(async () => {
          const response = await adminApi.deleteGroup(groupId);
          if (!response.ok) throw new Error("Failed to delete group");
          await this.fetchGroups();
        }),
      noCallback: () => {},
    });
  }

  async runAction(action) {
    try {
      loadingScreenManager.showLoadingScreen();
      this.error.textContent = "";
      await action();
    } catch (error) {
      this.error.textContent = error.message ?? String(error);
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }
}

customElements.define("admin-groups-page", AdminGroupsPage);
