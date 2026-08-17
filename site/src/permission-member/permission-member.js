import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";

// Ordered to match server/src/models.rs's PermissionFlags field order, so the toggle grid
// reads the same top-to-bottom as the struct it's editing.
const PERMISSION_LABELS = [
  ["invite_members", "Invite members"],
  ["regenerate_group_key", "Regenerate group token"],
  ["kick_members", "Kick members"],
  ["manage_settings", "Manage settings"],
  ["manage_permissions", "Manage permissions"],
  ["post_map_markers", "Post map markers"],
  ["post_callouts", "Post callouts"],
  ["manage_goals", "Manage goals"],
  ["manage_discord", "Manage Discord"],
  ["manage_events", "Manage events"],
];

export class PermissionMember extends BaseElement {
  constructor() {
    super();
    this.permissionKeys = PERMISSION_LABELS.map(([key]) => key);
  }

  html() {
    return `{{permission-member.html}}`;
  }

  grantedCount() {
    return this.permissionKeys.filter((key) => this.permission[key]).length;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.head = this.querySelector(".permission-member__head");
    this.body = this.querySelector(".permission-member__body");
    this.arrow = this.querySelector(".permission-member__arrow");
    this.error = this.querySelector(".permission-member__error");
    this.renderToggles();

    this.eventListener(this.head, "click", this.toggleOpen.bind(this));
    const saveButton = this.querySelector(".permission-member__save");
    this.eventListener(saveButton, "click", this.save.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  renderToggles() {
    const grid = this.querySelector(".permission-member__grid");
    grid.innerHTML = "";
    for (const [key, label] of PERMISSION_LABELS) {
      const wrapper = document.createElement("div");
      wrapper.className = "permission-member__toggle";

      const inputId = `permission-member__${this.permission.account_id}-${key}`;
      const input = document.createElement("input");
      input.type = "checkbox";
      input.id = inputId;
      input.checked = !!this.permission[key];
      input.dataset.key = key;

      const inputLabel = document.createElement("label");
      inputLabel.setAttribute("for", inputId);
      inputLabel.textContent = label;

      wrapper.appendChild(input);
      wrapper.appendChild(inputLabel);
      grid.appendChild(wrapper);
    }
  }

  toggleOpen() {
    this.body.classList.toggle("permission-member__body--open");
    this.arrow.classList.toggle("permission-member__arrow--open");
  }

  hideError() {
    this.error.innerHTML = "";
  }

  showError(message) {
    this.error.innerHTML = message;
  }

  async save() {
    this.hideError();
    const patch = {};
    for (const input of this.querySelectorAll(".permission-member__grid input[type=checkbox]")) {
      const key = input.dataset.key;
      if (input.checked !== !!this.permission[key]) {
        patch[key] = input.checked;
      }
    }

    if (Object.keys(patch).length === 0) return;

    try {
      loadingScreenManager.showLoadingScreen();
      const result = await api.updateGroupPermissions(this.permission.account_id, patch);
      if (result.ok) {
        this.permission = await result.json();
        const count = this.querySelector(".permission-member__count");
        count.textContent = `${this.grantedCount()} / ${this.permissionKeys.length}`;
      } else {
        const message = await result.text();
        this.showError(`Failed to save permissions: ${message}`);
      }
    } catch (error) {
      this.showError(`Failed to save permissions: ${error}`);
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }
}

customElements.define("permission-member", PermissionMember);
