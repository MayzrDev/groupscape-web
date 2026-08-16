import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { requireAdmin } from "../data/admin-guard";

export class AdminAccountsPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-accounts-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.init();
  }

  async init() {
    if (!(await requireAdmin())) return;
    try {
      const response = await adminApi.accountsSummary();
      if (!response.ok) return;
      const data = await response.json();
      this.querySelector(".admin-accounts__count").textContent = data.count;
    } catch {
      // Leave the placeholder count in place if the summary can't be fetched.
    }
  }
}

customElements.define("admin-accounts-page", AdminAccountsPage);
