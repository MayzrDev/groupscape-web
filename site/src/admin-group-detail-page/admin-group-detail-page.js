import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { adminStorage } from "../data/admin-storage";
import { adminViewSession } from "../data/admin-view-session";
import { requireAdmin } from "../data/admin-guard";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";

const MEMBER_DATA_COLUMNS = [
  { key: "collection_log", label: "collection log" },
  { key: "combat_achievements", label: "combat achievements" },
  { key: "skill_xp_history", label: "skill & XP history" },
  { key: "bank_value_history", label: "bank value history" },
];

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
    this.eventListener(
      this.querySelector(".admin-group-detail__view-as-member"),
      "click",
      this.viewAsMember.bind(this)
    );
    this.eventListener(this.querySelector(".admin-data-mgmt__clear-logs"), "click", this.clearLogs.bind(this));
    this.eventListener(
      this.querySelector(".admin-data-mgmt__clear-selected"),
      "click",
      this.clearSelectedMemberData.bind(this)
    );
    this.querySelectorAll(".admin-data-mgmt__selectall").forEach((button) => {
      this.eventListener(button, "click", () => this.toggleSelectAll(button.dataset.col));
    });
    await this.fetchGroup();
  }

  // Opens the group's real dashboard as a read-only observer - see `admin-view-session.js` and
  // the server's `/api/admin/group-view/{group_id}` scope. Deliberately doesn't touch the
  // group's own token/session state, so members never see that an admin looked.
  async viewAsMember() {
    const error = this.querySelector(".admin-group-detail__error");
    try {
      const response = await adminApi.viewGroup(this.groupId);
      if (!response.ok) {
        error.textContent = "Failed to open group view.";
        return;
      }
      adminViewSession.start(this.groupId, this.group?.group_name ?? "", adminStorage.getAdminToken());
      window.history.pushState("", "", "/group");
    } catch (e) {
      error.textContent = `Failed to open group view: ${e}`;
    }
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
        ? group.members.map((member) => `<div class="admin-roster__member">${member.member_name}</div>`).join("")
        : `<div class="admin-roster__empty">No members.</div>`;
    this.querySelector(".admin-group-detail__roster-count").textContent = group.members.length;

    this.renderMemberDataTable();
  }

  renderMemberDataTable() {
    const tbody = this.querySelector(".admin-data-mgmt__tbody");
    const members = this.group.members;
    tbody.innerHTML =
      members.length > 0
        ? members
            .map(
              (member) => `
      <tr>
        <td>${member.member_name}</td>
        ${MEMBER_DATA_COLUMNS.map((col) => {
          const id = `admin-data-mgmt__cb-${member.member_id}-${col.key}`;
          return `<td><input type="checkbox" id="${id}" data-member-id="${member.member_id}" data-col="${col.key}"><label for="${id}"></label></td>`;
        }).join("")}
      </tr>
    `
            )
            .join("")
        : `<tr><td colspan="${MEMBER_DATA_COLUMNS.length + 1}">No members.</td></tr>`;
    this.eventListener(tbody, "change", this.refreshSelectionState.bind(this));
    this.refreshSelectionState();
  }

  getSelectedMemberData() {
    return Array.from(this.querySelectorAll(".admin-data-mgmt__tbody input[type='checkbox']:checked")).map(
      (checkbox) => ({
        member_id: Number(checkbox.dataset.memberId),
        data_type: checkbox.dataset.col,
      })
    );
  }

  refreshSelectionState() {
    const selected = this.getSelectedMemberData();
    const clearButton = this.querySelector(".admin-data-mgmt__clear-selected");
    const countLabel = this.querySelector(".admin-data-mgmt__selection-count");
    clearButton.disabled = selected.length === 0;
    countLabel.textContent = selected.length === 0 ? "Nothing selected" : `${selected.length} selected`;
  }

  toggleSelectAll(col) {
    const boxes = Array.from(this.querySelectorAll(`.admin-data-mgmt__tbody input[data-col="${col}"]`));
    const allChecked = boxes.length > 0 && boxes.every((box) => box.checked);
    boxes.forEach((box) => (box.checked = !allChecked));
    this.refreshSelectionState();
  }

  clearLogs() {
    confirmDialogManager.confirm({
      headline: "Clear logs?",
      body: `Every activity feed and loot log entry for ${this.group.group_name} will be permanently deleted. This cannot be undone.`,
      yesCallback: () =>
        this.runAction(async () => {
          const response = await adminApi.clearLogs(this.groupId);
          if (!response.ok) throw new Error("Failed to clear logs");
        }),
      noCallback: () => {},
    });
  }

  clearSelectedMemberData() {
    const items = this.getSelectedMemberData();
    if (items.length === 0) return;

    const byMember = new Map();
    for (const { member_id, data_type } of items) {
      const member = this.group.members.find((m) => m.member_id === member_id);
      const label = MEMBER_DATA_COLUMNS.find((c) => c.key === data_type).label;
      const name = member ? member.member_name : `#${member_id}`;
      if (!byMember.has(name)) byMember.set(name, []);
      byMember.get(name).push(label);
    }
    const summary = Array.from(byMember.entries())
      .map(([name, types]) => `${name}: ${types.join(", ")}`)
      .join("\n");

    confirmDialogManager.confirm({
      headline: `Clear data for ${items.length} selection${items.length === 1 ? "" : "s"}?`,
      body: `This cannot be undone.\n${summary}`,
      yesCallback: () =>
        this.runAction(async () => {
          const response = await adminApi.clearMemberData(this.groupId, items);
          if (!response.ok) throw new Error("Failed to clear member data");
        }),
      noCallback: () => {},
    });
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
