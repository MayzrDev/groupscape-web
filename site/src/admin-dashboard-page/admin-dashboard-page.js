import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { requireAdmin } from "../data/admin-guard";

export class AdminDashboardPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-dashboard-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.error = this.querySelector(".admin-dashboard__error");
    this.searchInput = this.querySelector(".admin-dashboard__search");
    this.searchResults = this.querySelector(".admin-dashboard__search-results");
    this.auditRows = this.querySelector(".admin-dashboard__audit-rows");

    this.eventListener(this.searchInput, "input", this.handleSearchInput.bind(this));

    this.init();
  }

  async init() {
    if (!(await requireAdmin())) return;
    await this.fetchDashboard();
  }

  async fetchDashboard() {
    try {
      const response = await adminApi.dashboard();
      if (!response.ok) {
        this.error.textContent = "Failed to load the dashboard.";
        return;
      }
      const data = await response.json();
      this.error.textContent = "";
      this.renderStats(data);
      this.renderAudit(data.recent_audit);
    } catch (e) {
      this.error.textContent = `Failed to load the dashboard: ${e}`;
    }
  }

  renderStats(data) {
    this.querySelector(".admin-dashboard__total").textContent = data.total;
    this.querySelector(".admin-dashboard__active").textContent = data.active;
    this.querySelector(".admin-dashboard__suspended").textContent = data.suspended;
    this.querySelector(".admin-dashboard__banned").textContent = data.banned;
    this.querySelector(".admin-dashboard__deleted").textContent = data.deleted;
    this.querySelector(".admin-dashboard__live-sessions").textContent = data.live_sessions;
    this.querySelector(".admin-dashboard__locked-out").textContent = data.locked_out;
  }

  renderAudit(entries) {
    if (entries.length === 0) {
      this.auditRows.innerHTML = `<tr><td colspan="4" class="admin-audit-log__empty">No audit entries yet.</td></tr>`;
      return;
    }
    this.auditRows.innerHTML = entries
      .map(
        (entry) => `
      <tr>
        <td class="admin-mono">${new Date(entry.created_at).toLocaleString()}</td>
        <td>${entry.action}</td>
        <td>${entry.target_type ?? ""}${entry.target_id ? ` #${entry.target_id}` : ""}</td>
        <td class="admin-mono admin-audit-log__detail">${entry.detail ? JSON.stringify(entry.detail) : ""}</td>
      </tr>
    `
      )
      .join("");
  }

  handleSearchInput() {
    clearTimeout(this.searchDebounce);
    const q = this.searchInput.value.trim();
    if (!q) {
      this.searchResults.hidden = true;
      return;
    }
    this.searchDebounce = setTimeout(() => this.runSearch(q), 250);
  }

  async runSearch(q) {
    try {
      const response = await adminApi.search(q);
      if (!response.ok) return;
      const data = await response.json();
      this.renderSearchResults(data);
    } catch {
      // Search is a convenience widget - fail quietly.
    }
  }

  renderSearchResults(data) {
    this.searchResults.hidden = false;
    if (data.accounts.length === 0 && data.groups.length === 0) {
      this.searchResults.innerHTML = `<span class="admin-dashboard__search-empty">No matches.</span>`;
      return;
    }

    let html = "";
    if (data.accounts.length > 0) {
      html += `<div class="admin-dashboard__search-group-heading">Accounts</div>`;
      html += data.accounts
        .map(
          (account) => `
        <a class="admin-dashboard__search-row" href="/admin/accounts?accountId=${account.id}">
          <span>${account.username ?? `Account #${account.id}`}</span>
          <span class="admin-badge admin-badge--${
            account.status === "active" ? "good" : account.status === "suspended" ? "warn" : "bad"
          }">${account.status}</span>
        </a>
      `
        )
        .join("");
    }
    if (data.groups.length > 0) {
      html += `<div class="admin-dashboard__search-group-heading">Groups</div>`;
      html += data.groups
        .map(
          (group) => `
        <a class="admin-dashboard__search-row" href="/admin/group-detail?groupId=${group.group_id}">
          <span>${group.group_name}</span>
          <span class="admin-badge admin-badge--${
            group.status === "active" ? "good" : group.status === "suspended" ? "warn" : "bad"
          }">${group.status}</span>
        </a>
      `
        )
        .join("");
    }
    this.searchResults.innerHTML = html;

    for (const link of this.searchResults.querySelectorAll("a")) {
      this.eventListener(link, "click", (event) => {
        event.preventDefault();
        window.history.pushState("", "", link.getAttribute("href"));
      });
    }
  }
}

customElements.define("admin-dashboard-page", AdminDashboardPage);
