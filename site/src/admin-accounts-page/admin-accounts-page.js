import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { requireAdmin } from "../data/admin-guard";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";

const PAGE_SIZE = 25;

/**
 * "Dialogue Split" layout: a persistent master list (left) + a detail pane (right) styled as
 * an OSRS NPC dialogue box, with numbered action options - the winning variant from the
 * Steward's Desk artifact prototype, ported onto this app's real component base class and
 * shared primitives (`confirmDialogManager`, `runAction`) rather than the artifact's
 * from-scratch approach.
 */
export class AdminAccountsPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-accounts-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.search = "";
    this.status = "";
    this.page = 1;
    this.accounts = [];
    this.total = 0;
    this.selectedAccountId = null;
    this.detail = null;
    this.render();

    this.searchInput = this.querySelector(".admin-accounts__search");
    this.statusFilter = this.querySelector(".admin-accounts__status-filter");
    this.rows = this.querySelector(".admin-accounts__rows");
    this.pager = this.querySelector(".admin-accounts__pager");
    this.listError = this.querySelector(".admin-accounts__error");

    this.emptyState = this.querySelector(".admin-accounts__empty-state");
    this.dialogue = this.querySelector(".admin-accounts__dialogue");
    this.dialogueTitle = this.querySelector(".admin-accounts__dialogue-title");
    this.dialogueMeta = this.querySelector(".admin-accounts__dialogue-meta");
    this.dialogueError = this.querySelector(".admin-accounts__dialogue-error");
    this.tempPasswordPanel = this.querySelector(".admin-accounts__temp-password");
    this.tempPasswordValue = this.querySelector(".admin-accounts__temp-password-value");
    this.emailForm = this.querySelector(".admin-accounts__email-form");
    this.emailInput = this.querySelector(".admin-accounts__email-input");
    this.groupsList = this.querySelector(".admin-accounts__groups");
    this.groupCount = this.querySelector(".admin-accounts__group-count");
    this.charactersList = this.querySelector(".admin-accounts__characters");
    this.characterCount = this.querySelector(".admin-accounts__character-count");
    this.sessionsList = this.querySelector(".admin-accounts__sessions");
    this.sessionCount = this.querySelector(".admin-accounts__session-count");
    this.optionsList = this.querySelector(".admin-accounts__options-list");

    this.eventListener(this.searchInput, "input", this.handleSearchInput.bind(this));
    this.eventListener(this.statusFilter, "change", this.handleStatusChange.bind(this));
    this.eventListener(this.querySelector(".js-prev-page"), "click", this.prevPage.bind(this));
    this.eventListener(this.querySelector(".js-next-page"), "click", this.nextPage.bind(this));
    this.eventListener(
      this.querySelector(".js-dismiss-temp-password"),
      "click",
      () => (this.tempPasswordPanel.hidden = true)
    );
    this.eventListener(this.querySelector(".js-save-email"), "click", this.saveEmail.bind(this));
    this.eventListener(this.querySelector(".js-cancel-email"), "click", () => (this.emailForm.hidden = true));

    this.init();
  }

  async init() {
    if (!(await requireAdmin())) return;
    await this.fetchAccounts();

    const preselectedAccountId = new URLSearchParams(window.location.search).get("accountId");
    if (preselectedAccountId) {
      await this.selectAccount(preselectedAccountId);
    }
  }

  handleSearchInput() {
    clearTimeout(this.searchDebounce);
    this.searchDebounce = setTimeout(() => {
      this.search = this.searchInput.value.trim();
      this.page = 1;
      this.fetchAccounts();
    }, 300);
  }

  handleStatusChange() {
    this.status = this.statusFilter.value;
    this.page = 1;
    this.fetchAccounts();
  }

  async fetchAccounts() {
    try {
      const response = await adminApi.listAccounts(this.search, this.status, null, this.page, PAGE_SIZE);
      if (!response.ok) {
        this.listError.textContent = "Failed to load accounts.";
        return;
      }
      const data = await response.json();
      this.accounts = data.accounts;
      this.total = data.total;
      this.listError.textContent = "";
      this.renderList();
    } catch (error) {
      this.listError.textContent = `Failed to load accounts: ${error}`;
    }
  }

  renderList() {
    if (this.accounts.length === 0) {
      this.rows.innerHTML = `<tr><td colspan="3" class="admin-accounts__empty-row">No accounts found.</td></tr>`;
    } else {
      this.rows.innerHTML = this.accounts
        .map(
          (account) => `
        <tr class="admin-accounts__row${
          String(account.id) === String(this.selectedAccountId) ? " active" : ""
        }" data-account-id="${account.id}">
          <td class="admin-accounts__row-email">${account.email ?? "(no email)"}</td>
          <td>${this.statusBadge(account)}</td>
          <td class="admin-mono">${
            account.last_login_at ? new Date(account.last_login_at).toLocaleDateString() : "&mdash;"
          }</td>
        </tr>
      `
        )
        .join("");
    }

    for (const row of this.rows.querySelectorAll(".admin-accounts__row")) {
      this.eventListener(row, "click", () => this.selectAccount(row.dataset.accountId));
    }

    const totalPages = Math.max(1, Math.ceil(this.total / PAGE_SIZE));
    this.pager.querySelector(".admin-accounts__pager-label").textContent = `Page ${this.page} of ${totalPages}`;
    this.pager.querySelector(".js-prev-page").disabled = this.page <= 1;
    this.pager.querySelector(".js-next-page").disabled = this.page >= totalPages;
  }

  statusBadge(account) {
    const badgeClass = account.status === "active" ? "good" : account.status === "suspended" ? "warn" : "bad";
    const label = account.status.charAt(0).toUpperCase() + account.status.slice(1);
    const lockedBadge = account.locked_out ? ` <span class="admin-badge admin-badge--warn">Locked</span>` : "";
    return `<span class="admin-badge admin-badge--${badgeClass}">${label}</span>${lockedBadge}`;
  }

  prevPage() {
    if (this.page <= 1) return;
    this.page -= 1;
    this.fetchAccounts();
  }

  nextPage() {
    const totalPages = Math.max(1, Math.ceil(this.total / PAGE_SIZE));
    if (this.page >= totalPages) return;
    this.page += 1;
    this.fetchAccounts();
  }

  async selectAccount(accountId) {
    this.selectedAccountId = accountId;
    this.tempPasswordPanel.hidden = true;
    this.emailForm.hidden = true;
    this.renderList();
    await this.fetchDetail();
  }

  async fetchDetail() {
    if (!this.selectedAccountId) return;
    try {
      const response = await adminApi.getAccount(this.selectedAccountId);
      if (!response.ok) {
        this.dialogueError.textContent = "Failed to load account.";
        return;
      }
      this.detail = await response.json();
      this.dialogueError.textContent = "";
      this.renderDetail();
    } catch (error) {
      this.dialogueError.textContent = `Failed to load account: ${error}`;
    }
  }

  renderDetail() {
    const account = this.detail;
    this.emptyState.hidden = true;
    this.dialogue.hidden = false;

    this.dialogueTitle.textContent = account.email ?? `Account #${account.id}`;
    const created = new Date(account.created_at).toLocaleDateString();
    const lastLogin = account.last_login_at ? new Date(account.last_login_at).toLocaleString() : "never";
    this.dialogueMeta.innerHTML = `
      ${this.statusBadge(account)}
      <span>#${account.id}</span>
      <span>Joined ${created}</span>
      <span>Last login: ${lastLogin}</span>
    `;

    this.groupCount.textContent = account.groups.length;
    this.groupsList.innerHTML =
      account.groups.length > 0
        ? account.groups
            .map(
              (g) => `
        <div class="admin-accounts__row-item">
          <span>${g.group_name}${g.is_owner ? " &middot; owner" : ""}</span>
        </div>
      `
            )
            .join("")
        : `<div class="admin-accounts__empty-row">No groups.</div>`;

    this.characterCount.textContent = account.characters.length;
    this.charactersList.innerHTML =
      account.characters.length > 0
        ? account.characters
            .map(
              (c) => `
        <div class="admin-accounts__row-item">
          <span>${c.display_rsn}</span>
          <span class="admin-accounts__row-item-meta">${c.status}</span>
        </div>
      `
            )
            .join("")
        : `<div class="admin-accounts__empty-row">No linked characters.</div>`;

    this.sessionCount.textContent = account.session_count;
    this.renderSessions();

    this.renderOptions(account);
  }

  async renderSessions() {
    const account = this.detail;
    try {
      const response = await adminApi.listAccountSessions(account.id);
      if (!response.ok) return;
      const sessions = await response.json();
      this.sessionsList.innerHTML =
        sessions.length > 0
          ? sessions
              .map(
                (s) => `
        <div class="admin-accounts__row-item" data-session-id="${s.session_id}">
          <span>${s.ip ?? "unknown IP"}${s.user_agent ? ` &middot; ${s.user_agent.slice(0, 40)}` : ""}</span>
          <button class="admin-btn admin-btn--small js-revoke-session" data-session-id="${s.session_id}">Revoke</button>
        </div>
      `
              )
              .join("")
          : `<div class="admin-accounts__empty-row">No active sessions.</div>`;

      for (const button of this.sessionsList.querySelectorAll(".js-revoke-session")) {
        this.eventListener(button, "click", () => this.revokeSession(button.dataset.sessionId));
      }
    } catch {
      // Session list is a nice-to-have on the detail pane; leave it blank on failure.
    }
  }

  renderOptions(account) {
    const options = [];

    options.push({ label: "Reset password (temp password, forces change)", action: () => this.resetPassword() });

    if (account.status === "active") {
      options.push({ label: "Suspend account", action: () => this.setStatus("suspended") });
      options.push({ label: "Ban account", danger: true, confirm: true, action: () => this.setStatus("banned") });
    } else if (account.status === "suspended") {
      options.push({ label: "Reactivate account", action: () => this.setStatus("active") });
      options.push({ label: "Ban account", danger: true, confirm: true, action: () => this.setStatus("banned") });
    } else if (account.status === "banned") {
      options.push({ label: "Reactivate account", action: () => this.setStatus("active") });
    } else if (account.status === "deleted") {
      options.push({ label: "Restore account (undo soft delete)", action: () => this.setStatus("active") });
    }

    if (account.status !== "deleted") {
      options.push({
        label: "Soft-delete account (reversible)",
        danger: true,
        confirm: true,
        action: () => this.softDelete(),
      });
    }

    if (account.locked_out) {
      options.push({ label: "Clear login lockout", action: () => this.clearLockout() });
    }

    options.push({ label: "Change email", action: () => this.showEmailForm() });
    options.push({ label: "Revoke all sessions", action: () => this.revokeAllSessions() });
    options.push({
      label: "Hard-delete account (permanent, cannot be undone)",
      danger: true,
      confirm: true,
      action: () => this.hardDelete(),
    });

    this.optionsList.innerHTML = options
      .map(
        (option, index) => `
      <li>
        <button class="admin-accounts__option-btn${
          option.danger ? " admin-accounts__option-btn--danger" : ""
        }" data-option-index="${index}">
          ${option.label}
        </button>
      </li>
    `
      )
      .join("");

    for (const button of this.optionsList.querySelectorAll(".admin-accounts__option-btn")) {
      const option = options[Number(button.dataset.optionIndex)];
      this.eventListener(button, "click", () => option.action());
    }
  }

  showEmailForm() {
    this.emailInput.value = this.detail.email ?? "";
    this.emailForm.hidden = false;
  }

  async saveEmail() {
    const email = this.emailInput.value.trim();
    if (!email) return;
    await this.runAction(async () => {
      const response = await adminApi.setAccountEmail(this.detail.id, email);
      if (!response.ok) {
        if (response.status === 409) throw new Error("That email is already registered.");
        throw new Error("Failed to update email.");
      }
      this.emailForm.hidden = true;
      await this.fetchDetail();
      await this.fetchAccounts();
    });
  }

  async resetPassword() {
    await this.runAction(async () => {
      const response = await adminApi.resetAccountPassword(this.detail.id);
      if (!response.ok) throw new Error("Failed to reset password.");
      const { temp_password } = await response.json();
      this.tempPasswordValue.textContent = temp_password;
      this.tempPasswordPanel.hidden = false;
      await this.fetchDetail();
    });
  }

  setStatus(status) {
    const run = () =>
      this.runAction(async () => {
        const response = await adminApi.setAccountStatus(this.detail.id, status);
        if (!response.ok) throw new Error(`Failed to set account status to ${status}.`);
        await this.fetchDetail();
        await this.fetchAccounts();
      });

    if (status === "banned") {
      confirmDialogManager.confirm({
        headline: `Ban ${this.detail.email ?? "this account"}?`,
        body: "This removes all of their group memberships (transferring ownership of any owned group first) and revokes their sessions.",
        yesCallback: run,
        noCallback: () => {},
      });
      return;
    }
    run();
  }

  softDelete() {
    confirmDialogManager.confirm({
      headline: `Soft-delete ${this.detail.email ?? "this account"}?`,
      body: "The account is deactivated and its email is freed up, but this can be undone later.",
      yesCallback: () =>
        this.runAction(async () => {
          const response = await adminApi.softDeleteAccount(this.detail.id);
          if (!response.ok) throw new Error("Failed to soft-delete account.");
          await this.fetchDetail();
          await this.fetchAccounts();
        }),
      noCallback: () => {},
    });
  }

  hardDelete() {
    confirmDialogManager.confirm({
      headline: `Permanently delete ${this.detail.email ?? "this account"}?`,
      body: "This removes the account, its characters, and its group memberships forever. This cannot be undone.",
      yesCallback: () =>
        this.runAction(async () => {
          const response = await adminApi.hardDeleteAccount(this.detail.id);
          if (!response.ok) throw new Error("Failed to hard-delete account.");
          this.selectedAccountId = null;
          this.detail = null;
          this.dialogue.hidden = true;
          this.emptyState.hidden = false;
          await this.fetchAccounts();
        }),
      noCallback: () => {},
    });
  }

  async clearLockout() {
    await this.runAction(async () => {
      const response = await adminApi.clearAccountLockout(this.detail.id);
      if (!response.ok) throw new Error("Failed to clear lockout.");
      await this.fetchDetail();
      await this.fetchAccounts();
    });
  }

  async revokeSession(sessionId) {
    await this.runAction(async () => {
      const response = await adminApi.revokeAccountSession(this.detail.id, sessionId);
      if (!response.ok) throw new Error("Failed to revoke session.");
      await this.fetchDetail();
    });
  }

  async revokeAllSessions() {
    await this.runAction(async () => {
      const response = await adminApi.revokeAllAccountSessions(this.detail.id);
      if (!response.ok) throw new Error("Failed to revoke sessions.");
      await this.fetchDetail();
    });
  }

  async runAction(action) {
    try {
      loadingScreenManager.showLoadingScreen();
      this.dialogueError.textContent = "";
      await action();
    } catch (error) {
      this.dialogueError.textContent = error.message ?? String(error);
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }
}

customElements.define("admin-accounts-page", AdminAccountsPage);
