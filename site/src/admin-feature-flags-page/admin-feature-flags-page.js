import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { requireAdmin } from "../data/admin-guard";

export class AdminFeatureFlagsPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-feature-flags-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.flags = [];
    this.render();

    this.list = this.querySelector(".admin-flags__list");
    this.error = this.querySelector(".admin-flags__error");
    this.newKeyInput = this.querySelector(".admin-flags__new-key");
    this.newDescInput = this.querySelector(".admin-flags__new-desc");
    this.addButton = this.querySelector(".admin-flags__add");
    this.eventListener(this.addButton, "click", this.addFlag.bind(this));

    this.init();
  }

  async init() {
    if (!(await requireAdmin())) return;
    await this.fetchFlags();
  }

  async fetchFlags() {
    try {
      const response = await adminApi.listFeatureFlags();
      if (!response.ok) {
        this.error.textContent = "Failed to load feature flags.";
        return;
      }
      this.flags = await response.json();
      this.error.textContent = "";
      this.renderFlags();
    } catch (e) {
      this.error.textContent = `Failed to load feature flags: ${e}`;
    }
  }

  renderFlags() {
    if (this.flags.length === 0) {
      this.list.innerHTML = `<div class="admin-flags__empty">No feature flags yet.</div>`;
      return;
    }

    this.list.innerHTML = this.flags
      .map(
        (flag) => `
      <div class="admin-flag-row" data-flag-key="${flag.flag_key}">
        <span class="admin-flag-row__key">${flag.flag_key}</span>
        <span class="admin-flag-row__desc">${flag.description ?? ""}</span>
        <button class="admin-toggle ${flag.enabled ? "admin-toggle--on" : ""}" role="switch" aria-checked="${
          flag.enabled
        }"></button>
      </div>
    `
      )
      .join("");

    for (const row of this.list.querySelectorAll(".admin-flag-row")) {
      const toggle = row.querySelector(".admin-toggle");
      this.eventListener(toggle, "click", () => this.toggleFlag(row.dataset.flagKey));
    }
  }

  async toggleFlag(flagKey) {
    const flag = this.flags.find((f) => f.flag_key === flagKey);
    if (!flag) return;
    try {
      const response = await adminApi.setFeatureFlag(flagKey, !flag.enabled, flag.description);
      if (!response.ok) {
        this.error.textContent = `Failed to update ${flagKey}.`;
        return;
      }
      this.error.textContent = "";
      await this.fetchFlags();
    } catch (e) {
      this.error.textContent = `Failed to update ${flagKey}: ${e}`;
    }
  }

  async addFlag() {
    const key = this.newKeyInput.value.trim();
    const description = this.newDescInput.value.trim();
    if (!key) {
      this.error.textContent = "Flag key is required.";
      return;
    }

    try {
      this.addButton.disabled = true;
      const response = await adminApi.setFeatureFlag(key, false, description);
      if (!response.ok) {
        this.error.textContent = "Failed to add flag.";
        return;
      }
      this.error.textContent = "";
      this.newKeyInput.value = "";
      this.newDescInput.value = "";
      await this.fetchFlags();
    } catch (e) {
      this.error.textContent = `Failed to add flag: ${e}`;
    } finally {
      this.addButton.disabled = false;
    }
  }
}

customElements.define("admin-feature-flags-page", AdminFeatureFlagsPage);
