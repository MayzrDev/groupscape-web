import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { requireAdmin } from "../data/admin-guard";

const PAGE_SIZE = 30;

export class AdminAuditLogPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-audit-log-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.page = 1;
    this.total = 0;
    this.render();

    this.tbody = this.querySelector(".admin-audit-log__rows");
    this.error = this.querySelector(".admin-audit-log__error");
    this.pager = this.querySelector(".admin-audit-log__pager");
    this.eventListener(this.querySelector(".js-prev-page"), "click", this.prevPage.bind(this));
    this.eventListener(this.querySelector(".js-next-page"), "click", this.nextPage.bind(this));

    this.init();
  }

  async init() {
    if (!(await requireAdmin())) return;
    await this.fetchEntries();
  }

  async fetchEntries() {
    try {
      const response = await adminApi.listAuditLog(this.page, PAGE_SIZE);
      if (!response.ok) {
        this.error.textContent = "Failed to load the audit log.";
        return;
      }
      const data = await response.json();
      this.error.textContent = "";
      this.renderEntries(data.entries, data.total);
    } catch (e) {
      this.error.textContent = `Failed to load the audit log: ${e}`;
    }
  }

  renderEntries(entries, total) {
    this.total = total;

    if (entries.length === 0) {
      this.tbody.innerHTML = `<tr><td colspan="4" class="admin-audit-log__empty">No audit entries yet.</td></tr>`;
    } else {
      this.tbody.innerHTML = entries
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

    const totalPages = Math.max(1, Math.ceil(this.total / PAGE_SIZE));
    this.pager.querySelector(".admin-pager__label").textContent = `Page ${this.page} of ${totalPages}`;
    this.pager.querySelector(".js-prev-page").disabled = this.page <= 1;
    this.pager.querySelector(".js-next-page").disabled = this.page >= totalPages;
  }

  prevPage() {
    if (this.page <= 1) return;
    this.page -= 1;
    this.fetchEntries();
  }

  nextPage() {
    const totalPages = Math.max(1, Math.ceil(this.total / PAGE_SIZE));
    if (this.page >= totalPages) return;
    this.page += 1;
    this.fetchEntries();
  }
}

customElements.define("admin-audit-log-page", AdminAuditLogPage);
